//! Proleptic Gregorian calendar dates: the little date math a month view
//! needs. Day numbers follow Howard Hinnant's `days_from_civil`.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A day of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Weekday {
    #[default]
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl Weekday {
    const ALL: [Self; 7] = [
        Self::Sunday,
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
    ];

    /// Days after Sunday.
    pub fn index(self) -> u8 {
        self as u8
    }

    fn from_index(index: i64) -> Self {
        Self::ALL[index.rem_euclid(7) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sunday => "Sunday",
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
        }
    }

    /// The day `offset` days after this one.
    pub fn add(self, offset: i64) -> Self {
        Self::from_index(i64::from(self.index()) + offset)
    }
}

/// A calendar date with no time or time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CalendarDate {
    /// The date, or `None` when the month or day does not exist.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        ((1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month))
            .then_some(Self { year, month, day })
    }

    /// Today in the local time zone, from the system clock. Without the
    /// platform's local time (outside unix), today in UTC.
    pub fn today() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs() as i64);
        local_date(seconds).unwrap_or_else(|| Self::from_days(seconds.div_euclid(86_400)))
    }

    pub fn year(self) -> i32 {
        self.year
    }

    /// 1 for January to 12 for December.
    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday.
        Weekday::Thursday.add(self.to_days())
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_days(self.to_days() + days)
    }

    /// The same day `months` months on, or the month's last day when it is
    /// shorter.
    pub fn add_months(self, months: i32) -> Self {
        let index = self.year * 12 + i32::from(self.month) - 1 + months;
        let (year, month) = (index.div_euclid(12), (index.rem_euclid(12) + 1) as u8);
        Self {
            year,
            month,
            day: self.day.min(days_in_month(year, month)),
        }
    }

    pub fn first_of_month(self) -> Self {
        Self { day: 1, ..self }
    }

    pub fn days_in_month(self) -> u8 {
        days_in_month(self.year, self.month)
    }

    pub fn month_name(self) -> &'static str {
        MONTH_NAMES[usize::from(self.month - 1)]
    }

    /// "Nov 15, 2026".
    pub fn short_label(self) -> String {
        format!("{} {}, {}", &self.month_name()[..3], self.day, self.year)
    }

    /// "Sunday, November 15, 2026".
    pub fn long_label(self) -> String {
        format!(
            "{}, {} {}, {}",
            self.weekday().name(),
            self.month_name(),
            self.day,
            self.year
        )
    }

    /// Days since 1970-01-01.
    fn to_days(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let month = i64::from(self.month);
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }

    fn from_days(days: i64) -> Self {
        let days = days + 719_468;
        let era = days.div_euclid(146_097);
        let day_of_era = days - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u8;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        } as u8;
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        Self {
            year: year as i32,
            month,
            day,
        }
    }
}

/// ISO 8601: "2026-11-15".
impl fmt::Display for CalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// The local date at `seconds` after the epoch.
#[cfg(unix)]
fn local_date(seconds: i64) -> Option<CalendarDate> {
    let time = libc::time_t::try_from(seconds).ok()?;
    // SAFETY: `tm` is plain data that `localtime_r` fills in; the
    // reentrant variant writes only to it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&time, &mut tm) }.is_null() {
        return None;
    }
    CalendarDate::new(
        tm.tm_year + 1900,
        u8::try_from(tm.tm_mon + 1).ok()?,
        u8::try_from(tm.tm_mday).ok()?,
    )
}

#[cfg(not(unix))]
fn local_date(_seconds: i64) -> Option<CalendarDate> {
    None
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::new(year, month, day).expect("valid date")
    }

    #[test]
    fn invalid_dates_are_rejected() {
        assert!(CalendarDate::new(2026, 2, 29).is_none());
        assert!(CalendarDate::new(2024, 2, 29).is_some());
        assert!(CalendarDate::new(1900, 2, 29).is_none());
        assert!(CalendarDate::new(2000, 2, 29).is_some());
        assert!(CalendarDate::new(2026, 13, 1).is_none());
        assert!(CalendarDate::new(2026, 4, 31).is_none());
    }

    #[test]
    fn day_numbers_round_trip_and_know_the_weekday() {
        assert_eq!(date(1970, 1, 1).to_days(), 0);
        assert_eq!(date(2000, 3, 1).to_days(), 11_017);
        for days in (-800_000..800_000).step_by(997) {
            assert_eq!(CalendarDate::from_days(days).to_days(), days);
        }
        assert_eq!(date(2026, 11, 15).weekday(), Weekday::Sunday);
        assert_eq!(date(2024, 2, 29).weekday(), Weekday::Thursday);
        assert_eq!(date(1969, 12, 31).weekday(), Weekday::Wednesday);
    }

    #[test]
    fn adding_days_and_months_crosses_boundaries() {
        assert_eq!(date(2026, 12, 31).add_days(1), date(2027, 1, 1));
        assert_eq!(date(2024, 3, 1).add_days(-1), date(2024, 2, 29));
        assert_eq!(date(2026, 1, 31).add_months(1), date(2026, 2, 28));
        assert_eq!(date(2026, 1, 15).add_months(-1), date(2025, 12, 15));
        assert_eq!(date(2026, 11, 15).add_months(14), date(2028, 1, 15));
    }

    #[test]
    fn today_is_within_a_day_of_utc() {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after the epoch")
            .as_secs() as i64;
        let utc = CalendarDate::from_days(seconds.div_euclid(86_400));
        let today = CalendarDate::today();
        assert!(today >= utc.add_days(-1) && today <= utc.add_days(1));
    }

    #[test]
    fn dates_format_for_people_and_iso() {
        let day = date(2026, 11, 5);
        assert_eq!(day.to_string(), "2026-11-05");
        assert_eq!(day.short_label(), "Nov 5, 2026");
        assert_eq!(day.long_label(), "Thursday, November 5, 2026");
    }
}
