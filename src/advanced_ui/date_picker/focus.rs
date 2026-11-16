//! Focus in a date picker's open calendar. The keyboard keeps focus on the
//! field and moves the active day; assistive technology puts focus on the
//! calendar or on the cell of a day. A cell's id names its day, not its place
//! in the grid, so it never comes to stand for another day when the month
//! turns.

use super::{Calendar, CalendarDate, DatePicker, GRID_DAYS};
use crate::core::ElementId;
use crate::core::geometry::Bounds;
use crate::elements::element::{Element, EventContext};
use std::any::TypeId;

/// The id of `picker`'s open calendar.
pub(super) fn grid_id(picker: ElementId) -> ElementId {
    ElementId::derived(picker, 0, TypeId::of::<Calendar>())
}

/// The id of the cell `picker`'s open calendar shows `date` in.
pub(super) fn day_id(picker: ElementId, date: CalendarDate) -> ElementId {
    ElementId::derived(picker, date.to_days() as u64, TypeId::of::<CalendarDate>())
}

impl DatePicker {
    /// The day the open calendar shows in the cell `id`.
    pub(super) fn day_of(&self, id: ElementId) -> Option<CalendarDate> {
        self.active?;
        let calendar = self.calendar(Bounds::default(), Bounds::default());
        (0..GRID_DAYS)
            .map(|index| calendar.day(index))
            .find(|&day| day_id(self.id.get(), day) == id)
    }

    /// Whether focus is on the field or, where assistive technology puts it,
    /// on the open calendar or one of its days.
    pub(super) fn has_focus(&self, cx: &EventContext) -> bool {
        cx.focused_id().is_some_and(|id| self.contains_id(id))
    }

    /// The day the keys move from and pick: the one assistive technology put
    /// focus on, else the active day.
    pub(super) fn current_day(&self, cx: &EventContext) -> Option<CalendarDate> {
        let focused = cx.focused_id().and_then(|id| self.day_of(id));
        focused.or(self.active)
    }
}
