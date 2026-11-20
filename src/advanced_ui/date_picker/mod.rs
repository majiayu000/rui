//! A date field that opens a month calendar in an overlay, after the
//! WAI-ARIA date picker dialog pattern: the arrow keys move a day or a week,
//! Page Up and Page Down a month (with Shift a year), Home and End to the
//! start and end of the week, Enter or Space picks, Escape closes.

mod date;
mod focus;

pub use date::{CalendarDate, Weekday};

use super::popover::paint_panel;
use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{
    ActionId, ActionOutcome, SYNC_ACCESSIBILITY_FOCUS_ACTION, StandardAction,
};
use crate::core::color::Color;
use crate::core::event::{KeyCode, KeyEvent, Modifiers};
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind, StableId,
    style_to_taffy,
};
use crate::elements::overlay::{Align, Placement, Side, place_anchored};
use crate::elements::text::TextAlign;
use crate::renderer::text::BaseDirection;
use crate::renderer::{Overlay, Primitive};
use focus::{day_id, grid_id};
use std::rc::Rc;
use taffy::prelude::*;

/// Days shown: six weeks always fit a month.
const GRID_DAYS: usize = 42;
const ICON_WIDTH: f32 = 16.0;

type DateChangeHandler = Box<dyn Fn(CalendarDate)>;
type DatePredicate = Rc<dyn Fn(CalendarDate) -> bool>;

/// A field showing the picked date; activating it opens a month calendar in
/// an overlay below it (above when there is no room below).
pub struct DatePicker {
    id: StableId,
    label: String,
    placeholder: String,
    value: Option<CalendarDate>,
    today: CalendarDate,
    min: Option<CalendarDate>,
    max: Option<CalendarDate>,
    disabled_dates: Option<DatePredicate>,
    week_start: Weekday,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    style: Style,
    on_change: Option<DateChangeHandler>,
    /// The day the open calendar's keyboard cursor is on; `None` closed.
    active: Option<CalendarDate>,
    hovered: Option<CalendarDate>,
}

impl DatePicker {
    pub fn new(label: impl Into<String>, value: Option<CalendarDate>) -> Self {
        let label = label.into();
        require_non_empty(&label, "date picker accessibility label must not be empty");
        Self {
            id: StableId::new(),
            label,
            placeholder: String::from("Select date"),
            value,
            today: CalendarDate::today(),
            min: None,
            max: None,
            disabled_dates: None,
            week_start: Weekday::default(),
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            style: Style::new(),
            on_change: None,
            active: None,
            hovered: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn value(mut self, value: Option<CalendarDate>) -> Self {
        self.value = value;
        self
    }

    /// Shown, and read to assistive technology, while no date is picked.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        let placeholder = placeholder.into();
        require_non_empty(&placeholder, "date picker placeholder must not be empty");
        self.placeholder = placeholder;
        self
    }

    /// The day marked as today, and opened on while no date is picked; today
    /// in the local time zone by default.
    pub fn today(mut self, today: CalendarDate) -> Self {
        self.today = today;
        self
    }

    /// The earliest date that can be picked.
    pub fn min(mut self, min: CalendarDate) -> Self {
        self.min = Some(min);
        self
    }

    /// The latest date that can be picked.
    pub fn max(mut self, max: CalendarDate) -> Self {
        self.max = Some(max);
        self
    }

    /// Dates for which `disabled` is true cannot be picked.
    pub fn disabled_dates(mut self, disabled: impl Fn(CalendarDate) -> bool + 'static) -> Self {
        self.disabled_dates = Some(Rc::new(disabled));
        self
    }

    /// The first column of the calendar; Sunday by default.
    pub fn week_start(mut self, week_start: Weekday) -> Self {
        self.week_start = week_start;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        if disabled {
            self.active = None;
        }
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(CalendarDate) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn selected_date(&self) -> Option<CalendarDate> {
        self.value
    }

    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// The day the open calendar's keyboard cursor is on.
    pub fn active_date(&self) -> Option<CalendarDate> {
        self.active
    }

    /// Whether `date` can be picked: inside the bounds and not disabled.
    pub fn is_selectable(&self, date: CalendarDate) -> bool {
        self.min.is_none_or(|min| date >= min)
            && self.max.is_none_or(|max| date <= max)
            && !self
                .disabled_dates
                .as_ref()
                .is_some_and(|disabled| disabled(date))
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    /// Keep the open calendar of the picker this one replaces, so it stays
    /// open across renders that do not change the value.
    pub(crate) fn reconcile_transient_state(&mut self, previous: &Self) {
        if self.state.can_activate() && previous.value == self.value {
            self.active = previous.active.map(|active| self.clamp(active));
            self.hovered = previous.hovered;
        }
    }

    fn clamp(&self, date: CalendarDate) -> CalendarDate {
        let date = self.min.map_or(date, |min| date.max(min));
        self.max.map_or(date, |max| date.min(max))
    }

    fn open(&mut self, cx: &EventContext) {
        if !self.state.can_activate() {
            return;
        }
        let active = self.clamp(self.value.unwrap_or(self.today));
        self.active = Some(active);
        cx.announce_accessibility_action(self.id.get(), active.long_label());
        cx.request_redraw();
    }

    fn close(&mut self, cx: &EventContext) {
        self.hovered = None;
        if self.active.take().is_some() {
            cx.request_redraw();
        }
    }

    /// Move the keyboard cursor to `date`, kept inside the bounds, and focus
    /// to the field, off a day assistive technology put it on: the day the
    /// keys then move from and pick is this one, which is read out.
    fn move_to(&mut self, date: CalendarDate, cx: &mut EventContext) {
        let date = self.clamp(date);
        if self.current_day(cx) != Some(date) {
            self.active = Some(date);
            cx.request_focus(Some(self.id.get()));
            cx.announce_accessibility_action(self.id.get(), date.long_label());
            cx.request_redraw();
        }
    }

    /// Step from the day focus is on, or open the calendar.
    fn move_by(&mut self, step: impl FnOnce(CalendarDate) -> CalendarDate, cx: &mut EventContext) {
        match self.current_day(cx) {
            Some(day) => self.move_to(step(day), cx),
            None => self.open(cx),
        }
    }

    /// Pick `date` when it can be picked, closing the calendar.
    fn pick(&mut self, date: CalendarDate, cx: &mut EventContext) -> bool {
        if !self.is_selectable(date) {
            return false;
        }
        self.close(cx);
        if self.value != Some(date) {
            self.value = Some(date);
            if let Some(handler) = &self.on_change {
                handler(date);
            }
        }
        cx.announce_accessibility_action(self.id.get(), format!("{} picked", date.long_label()));
        cx.request_focus(Some(self.id.get()));
        true
    }

    /// Enter and Space: open the calendar, or pick the day focus is on, which
    /// is its active day unless assistive technology put focus on another.
    fn activate(&mut self, cx: &mut EventContext) {
        match self.current_day(cx) {
            Some(day) => {
                self.pick(day, cx);
            }
            None => self.open(cx),
        }
    }

    fn trigger_width(&self, cx: &mut LayoutContext) -> f32 {
        let weight = self.theme.typography.label_weight;
        // The widest label of any month, so the field keeps its width.
        let year = self.value.unwrap_or(self.today).year();
        let widest = (1..=12)
            .filter_map(|month| CalendarDate::new(year, month, 28))
            .map(CalendarDate::short_label)
            .chain(std::iter::once(self.placeholder.clone()))
            .map(|label| self.theme.measure_label(cx, &label, self.size, weight))
            .fold(0.0, f32::max);
        widest + self.theme.horizontal_padding(self.size) * 2.0 + ICON_WIDTH
    }

    fn calendar(&self, trigger: Bounds, window: Bounds) -> Calendar {
        let cell = self.theme.control_height(ControlSize::Small);
        let padding = self.theme.control_gap();
        let size = Size::new(cell * 7.0 + padding * 2.0, cell * 8.0 + padding * 3.0);
        let bounds = place_anchored(
            trigger,
            size,
            window,
            Placement::new(Side::Bottom, Align::Start, padding * 0.5),
        );
        let month = self.active.unwrap_or(self.today).first_of_month();
        let lead =
            (i64::from(month.weekday().index()) - i64::from(self.week_start.index())).rem_euclid(7);
        Calendar {
            bounds,
            cell,
            padding,
            month,
            first: month.add_days(-lead),
        }
    }

    fn handle_calendar_press(
        &mut self,
        calendar: &Calendar,
        position: Point,
        cx: &mut EventContext,
    ) {
        // To the field first: the month turns from the one shown, not from
        // a day assistive technology put focus on.
        cx.request_focus(Some(self.id.get()));
        if calendar.previous().contains(position) {
            self.move_by(|date| date.add_months(-1), cx);
        } else if calendar.next().contains(position) {
            self.move_by(|date| date.add_months(1), cx);
        } else if let Some(date) = calendar.day_at(position) {
            self.pick(date, cx);
        }
    }

    fn paint_trigger(&self, cx: &mut PaintContext, trigger: Bounds) {
        let mut state: ControlState = self.state.into();
        state.focused = cx.is_focused(self.id.get()) && !state.disabled;
        cx.paint(Primitive::Quad {
            bounds: trigger,
            background: self.theme.surface_color_for_state(state).to_rgba(),
            border_color: self
                .theme
                .state_border_color(state, self.theme.colors.border)
                .to_rgba(),
            border_widths: Edges::all(if state.focused { 2.0 } else { 1.0 }),
            corner_radii: Corners::all(self.theme.control_radius()),
        });
        let mut color = self.theme.colors.text;
        if state.disabled {
            color = color.with_alpha(self.theme.state.disabled_opacity);
        } else if self.value.is_none() {
            color = color.with_alpha(self.theme.state.read_only_opacity);
        }
        let padding = self.theme.horizontal_padding(self.size);
        let content = self
            .value
            .map_or_else(|| self.placeholder.clone(), CalendarDate::short_label);
        let font_size = self.theme.text_size(self.size);
        let text = Bounds::from_xywh(
            trigger.x() + padding,
            trigger.y(),
            (trigger.width() - padding * 2.0 - ICON_WIDTH).max(0.0),
            trigger.height(),
        );
        let icon = Bounds::from_xywh(
            trigger.max_x() - padding - ICON_WIDTH,
            trigger.y(),
            ICON_WIDTH,
            trigger.height(),
        );
        paint_text(
            cx,
            &self.theme,
            text,
            content,
            color,
            font_size,
            TextAlign::Left,
        );
        paint_text(
            cx,
            &self.theme,
            icon,
            "▾".into(),
            color,
            font_size,
            TextAlign::Right,
        );
    }

    fn paint_calendar(&self, cx: &mut PaintContext, calendar: &Calendar) {
        let theme = &self.theme;
        paint_panel(cx, calendar.bounds, false, *theme);
        let text = theme.colors.text;
        let muted = text.with_alpha(theme.state.read_only_opacity);
        let font_size = theme.text_size(ControlSize::Small);
        let title = format!("{} {}", calendar.month.month_name(), calendar.month.year());
        paint_text(
            cx,
            theme,
            calendar.title(),
            title,
            text,
            font_size,
            TextAlign::Center,
        );
        for (bounds, glyph) in [(calendar.previous(), "‹"), (calendar.next(), "›")] {
            paint_text(
                cx,
                theme,
                bounds,
                glyph.into(),
                text,
                font_size,
                TextAlign::Center,
            );
        }
        for column in 0..7 {
            let name = self.week_start.add(column).name()[..2].to_string();
            let bounds = calendar.weekday_header(column as usize);
            paint_text(cx, theme, bounds, name, muted, font_size, TextAlign::Center);
        }
        let radius = Corners::all(theme.indicator_radius());
        for index in 0..GRID_DAYS {
            let date = calendar.day(index);
            let bounds = calendar.cell(index).inflate(-1.0, -1.0);
            let selected = self.value == Some(date);
            let active = self.active == Some(date);
            let selectable = self.is_selectable(date);
            let background = if selected {
                theme.colors.primary.rest.background
            } else if self.hovered == Some(date) && selectable {
                theme.state.hover_surface
            } else {
                Color::TRANSPARENT
            };
            let border = if active {
                theme.state.focus_ring
            } else if date == self.today {
                theme.colors.border
            } else {
                Color::TRANSPARENT
            };
            cx.paint(Primitive::Quad {
                bounds,
                background: background.to_rgba(),
                border_color: border.to_rgba(),
                border_widths: Edges::all(if active { 2.0 } else { 1.0 }),
                corner_radii: radius,
            });
            let color = if selected {
                theme.colors.text_on_accent
            } else if !selectable {
                text.with_alpha(theme.state.disabled_opacity * 0.6)
            } else if date.month() != calendar.month.month() {
                muted
            } else {
                text
            };
            paint_text(
                cx,
                theme,
                bounds,
                date.day().to_string(),
                color,
                font_size,
                TextAlign::Center,
            );
        }
    }

    fn grid_node(&self, active: CalendarDate) -> Result<AccessibilityNode, AccessibilityError> {
        let calendar = self.calendar(Bounds::default(), Bounds::default());
        let title = format!("{} {}", calendar.month.month_name(), calendar.month.year());
        let mut grid = AccessibilityNode::label_required(
            grid_id(self.id.get()),
            AccessibilityRole::Grid,
            title,
        )?
        .with_value(active.long_label());
        for index in 0..GRID_DAYS {
            let date = calendar.day(index);
            let selectable = self.is_selectable(date);
            let mut cell = AccessibilityNode::label_required(
                day_id(self.id.get(), date),
                AccessibilityRole::GridCell,
                date.long_label(),
            )?
            .with_selected(self.value == Some(date))
            .with_enabled(selectable);
            if selectable {
                cell = cell.with_action(AccessibilityAction::Activate);
            }
            grid = grid.with_child(cell);
        }
        Ok(grid)
    }
}

/// Where the open calendar's parts go.
struct Calendar {
    bounds: Bounds,
    cell: f32,
    padding: f32,
    /// The first day of the month shown.
    month: CalendarDate,
    /// The day in the first cell.
    first: CalendarDate,
}

impl Calendar {
    fn row(&self, row: usize) -> f32 {
        self.bounds.y() + self.padding * 2.0 + self.cell * row as f32
    }

    fn title(&self) -> Bounds {
        Bounds::from_xywh(
            self.bounds.x() + self.padding + self.cell,
            self.bounds.y() + self.padding,
            self.cell * 5.0,
            self.cell,
        )
    }

    fn previous(&self) -> Bounds {
        Bounds::from_xywh(
            self.bounds.x() + self.padding,
            self.bounds.y() + self.padding,
            self.cell,
            self.cell,
        )
    }

    fn next(&self) -> Bounds {
        Bounds::from_xywh(
            self.bounds.max_x() - self.padding - self.cell,
            self.bounds.y() + self.padding,
            self.cell,
            self.cell,
        )
    }

    fn weekday_header(&self, column: usize) -> Bounds {
        Bounds::from_xywh(
            self.bounds.x() + self.padding + self.cell * column as f32,
            self.row(1),
            self.cell,
            self.cell,
        )
    }

    fn cell(&self, index: usize) -> Bounds {
        Bounds::from_xywh(
            self.bounds.x() + self.padding + self.cell * (index % 7) as f32,
            self.row(2 + index / 7),
            self.cell,
            self.cell,
        )
    }

    fn day(&self, index: usize) -> CalendarDate {
        self.first.add_days(index as i64)
    }

    fn day_at(&self, position: Point) -> Option<CalendarDate> {
        (0..GRID_DAYS)
            .find(|&index| self.cell(index).contains(position))
            .map(|index| self.day(index))
    }
}

fn paint_text(
    cx: &mut PaintContext,
    theme: &Theme,
    bounds: Bounds,
    content: String,
    color: Color,
    font_size: f32,
    align: TextAlign,
) {
    cx.paint(Primitive::Text {
        bounds,
        content: content.into(),
        color: color.to_rgba(),
        font_size,
        font_weight: theme.typography.label_weight,
        font_family: None,
        font_features: Default::default(),
        line_height: theme.typography.line_height,
        align,
        direction: BaseDirection::Auto,
    });
}

impl Element for DatePicker {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.get())
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        Some(&mut self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.trigger_width(cx));
        style.size.height = Dimension::Length(self.theme.control_height(self.size));
        cx.new_leaf(style)
            .unwrap_or_else(|err| panic!("failed to create date picker layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let trigger = cx.bounds();
        cx.register_hit_region(self.id.get(), trigger);
        self.paint_trigger(cx, trigger);
        if self.active.is_none() {
            return;
        }
        let calendar = self.calendar(trigger, cx.window());
        let id = self.id.get();
        cx.paint_overlay(Overlay::popup(id, calendar.bounds), calendar.bounds, |cx| {
            cx.register_hit_region(id, calendar.bounds);
            self.paint_calendar(cx, &calendar);
        });
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let value = self
            .value
            .map_or_else(|| self.placeholder.clone(), CalendarDate::long_label);
        let mut node = AccessibilityNode::label_required(
            self.id.get(),
            AccessibilityRole::ComboBox,
            &self.label,
        )?
        .value_required(value)?
        .with_enabled(!self.state.disabled())
        .with_focused(cx.a11y_has_focus(self.id.get()));
        if self.state.can_activate() {
            node = node.with_action(AccessibilityAction::Activate);
        }
        if let Some(active) = self.active {
            node = node.with_child(self.grid_node(active)?);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let trigger = cx.layout_bounds();
        if self.active.is_some() {
            let calendar = self.calendar(trigger, cx.window());
            let in_calendar = cx
                .for_overlay(calendar.bounds)
                .contains_pointer(event.position);
            // The window shows every press to light-dismiss owners first;
            // only one outside the field and the calendar closes it.
            if cx.is_dismiss_pass() {
                if !in_calendar && !cx.contains_pointer(event.position) {
                    self.close(cx);
                }
                return false;
            }
            match event.kind {
                PointerEventKind::Move => {
                    let hovered = in_calendar
                        .then(|| calendar.day_at(event.position))
                        .flatten();
                    if hovered != self.hovered {
                        self.hovered = hovered;
                        cx.request_redraw();
                    }
                }
                PointerEventKind::Down if in_calendar => {
                    self.handle_calendar_press(&calendar, event.position, cx);
                    return true;
                }
                PointerEventKind::Up if in_calendar => return true,
                _ => {}
            }
        }

        let inside = cx.contains_pointer(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.update_hover_inside(inside, cx);
                false
            }
            PointerEventKind::Down => {
                if self.state.press_inside(inside, cx) {
                    cx.request_focus(Some(self.id.get()));
                    return true;
                }
                // A press anywhere else dismisses the calendar.
                self.close(cx);
                false
            }
            PointerEventKind::Up => {
                if !self.state.release_inside(inside, cx).activated {
                    return false;
                }
                if self.active.is_some() {
                    self.close(cx);
                } else {
                    self.open(cx);
                }
                true
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.state.can_activate() || !self.has_focus(cx) {
            return false;
        }
        let years = if event.modifiers == Modifiers::shift() {
            12
        } else {
            1
        };
        match event.key {
            KeyCode::Space if event.modifiers == Modifiers::none() => self.activate(cx),
            KeyCode::PageUp => self.move_by(|date| date.add_months(-years), cx),
            KeyCode::PageDown => self.move_by(|date| date.add_months(years), cx),
            KeyCode::Home if self.active.is_some() => {
                let week_start = self.week_start;
                self.move_by(|date| date.add_days(-days_into_week(date, week_start)), cx)
            }
            KeyCode::End if self.active.is_some() => {
                let week_start = self.week_start;
                self.move_by(
                    |date| date.add_days(6 - days_into_week(date, week_start)),
                    cx,
                )
            }
            _ => return false,
        }
        true
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if let ActionId::Custom(name) = action
            && name == SYNC_ACCESSIBILITY_FOCUS_ACTION
        {
            if !self.has_focus(cx) {
                self.close(cx);
            }
            return ActionOutcome::Ignored;
        }
        if !self.state.can_activate() || !self.has_focus(cx) {
            return ActionOutcome::Ignored;
        }
        match action {
            ActionId::Standard(StandardAction::MoveLeft) if self.active.is_some() => {
                self.move_by(|date| date.add_days(-1), cx)
            }
            ActionId::Standard(StandardAction::MoveRight) if self.active.is_some() => {
                self.move_by(|date| date.add_days(1), cx)
            }
            ActionId::Standard(StandardAction::MoveUp) if self.active.is_some() => {
                self.move_by(|date| date.add_days(-7), cx)
            }
            ActionId::Standard(StandardAction::MoveDown) => {
                self.move_by(|date| date.add_days(7), cx)
            }
            ActionId::Standard(StandardAction::Activate) => self.activate(cx),
            ActionId::Standard(StandardAction::Cancel) if self.active.is_some() => {
                self.close(cx);
                // Off a day of the calendar that went away.
                cx.request_focus(Some(self.id.get()));
            }
            _ => return ActionOutcome::Ignored,
        }
        ActionOutcome::handled("advanced_ui.date_picker")
    }

    fn contains_id(&self, id: ElementId) -> bool {
        let calendar = self.active.is_some() && id == grid_id(self.id.get());
        id == self.id.get() || calendar || self.day_of(id).is_some()
    }
}

/// How many days `date` is after the start of its week.
fn days_into_week(date: CalendarDate, week_start: Weekday) -> i64 {
    (i64::from(date.weekday().index()) - i64::from(week_start.index())).rem_euclid(7)
}

pub fn date_picker(label: impl Into<String>, value: Option<CalendarDate>) -> DatePicker {
    DatePicker::new(label, value)
}
