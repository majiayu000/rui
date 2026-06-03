use crate::advanced_ui::tokens::ControlState;
use crate::core::color::Color;
use crate::core::event::Cursor;
use crate::core::geometry::{Bounds, Point};
use crate::elements::element::EventContext;

pub const INVALID_BORDER_COLOR: Color = Color::Rgba(crate::core::color::Rgba::new(
    0.8627451, 0.14901961, 0.14901961, 1.0,
));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractionState {
    hovered: bool,
    pressed: bool,
    focused: bool,
    selected: bool,
    disabled: bool,
    read_only: bool,
    invalid: bool,
}

impl InteractionState {
    pub fn hovered(self) -> bool {
        self.hovered
    }

    pub fn pressed(self) -> bool {
        self.pressed
    }

    pub fn focused(self) -> bool {
        self.focused
    }

    pub fn selected(self) -> bool {
        self.selected
    }

    pub fn disabled(self) -> bool {
        self.disabled
    }

    pub fn read_only(self) -> bool {
        self.read_only
    }

    pub fn invalid(self) -> bool {
        self.invalid
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.clear_transient();
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        if read_only {
            self.clear_transient();
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn set_invalid(&mut self, invalid: bool) {
        self.invalid = invalid;
    }

    pub fn can_activate(self) -> bool {
        !self.disabled && !self.read_only
    }

    pub fn cursor(self) -> Cursor {
        if self.disabled {
            Cursor::NotAllowed
        } else if self.read_only {
            Cursor::Default
        } else {
            Cursor::Pointer
        }
    }

    pub fn control_state(self) -> ControlState {
        ControlState {
            hovered: self.hovered,
            pressed: self.pressed,
            selected: self.selected,
            disabled: self.disabled,
            read_only: self.read_only,
            invalid: self.invalid,
            focused: self.focused,
            loading: false,
            error: false,
        }
    }

    pub fn clear_transient(&mut self) {
        self.hovered = false;
        self.pressed = false;
    }

    pub fn update_hover(&mut self, bounds: Bounds, position: Point, cx: &EventContext) -> bool {
        if !self.can_activate() {
            let changed = self.hovered || self.pressed;
            self.clear_transient();
            if changed {
                cx.request_redraw();
            }
            return false;
        }

        let inside = bounds.contains(position);
        if self.hovered != inside {
            self.hovered = inside;
            cx.request_redraw();
        }
        if inside {
            cx.set_cursor(self.cursor());
        }
        inside
    }

    pub fn press_inside(&mut self, inside: bool, cx: &EventContext) -> bool {
        if !self.can_activate() || !inside {
            return false;
        }

        self.pressed = true;
        cx.request_redraw();
        true
    }

    pub fn release_inside(&mut self, inside: bool, cx: &EventContext) -> InteractionRelease {
        let was_pressed = self.pressed;
        self.pressed = false;
        if was_pressed {
            cx.request_redraw();
        }

        InteractionRelease {
            was_pressed,
            activated: self.can_activate() && inside && was_pressed,
        }
    }
}

impl From<InteractionState> for ControlState {
    fn from(value: InteractionState) -> Self {
        value.control_state()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionRelease {
    pub was_pressed: bool,
    pub activated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexedInteractionState {
    hovered_index: Option<usize>,
    pressed_index: Option<usize>,
}

impl IndexedInteractionState {
    pub fn hovered_index(self) -> Option<usize> {
        self.hovered_index
    }

    pub fn pressed_index(self) -> Option<usize> {
        self.pressed_index
    }

    pub fn clear(&mut self) {
        self.hovered_index = None;
        self.pressed_index = None;
    }

    pub fn update_hover(
        &mut self,
        index: Option<usize>,
        cx: &EventContext,
        state: InteractionState,
    ) {
        if !state.can_activate() {
            if self.hovered_index.is_some() || self.pressed_index.is_some() {
                self.clear();
                cx.request_redraw();
            }
            return;
        }

        if self.hovered_index != index {
            self.hovered_index = index;
            cx.request_redraw();
        }
        if index.is_some() {
            cx.set_cursor(state.cursor());
        }
    }

    pub fn press(
        &mut self,
        index: Option<usize>,
        cx: &EventContext,
        state: InteractionState,
    ) -> bool {
        if !state.can_activate() {
            self.clear();
            return false;
        }

        self.pressed_index = index;
        if index.is_some() {
            cx.request_redraw();
            true
        } else {
            false
        }
    }

    pub fn release(
        &mut self,
        index: Option<usize>,
        cx: &EventContext,
        state: InteractionState,
    ) -> IndexedInteractionRelease {
        let pressed_index = self.pressed_index;
        self.pressed_index = None;
        if pressed_index.is_some() {
            cx.request_redraw();
        }

        IndexedInteractionRelease {
            pressed_index,
            released_index: index,
            activated: state.can_activate() && pressed_index.is_some() && pressed_index == index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedInteractionRelease {
    pub pressed_index: Option<usize>,
    pub released_index: Option<usize>,
    pub activated: bool,
}

pub fn validation_border_color(invalid: bool, default: Color) -> Color {
    if invalid {
        INVALID_BORDER_COLOR
    } else {
        default
    }
}

pub fn require_non_empty(value: &str, message: &str) {
    if value.trim().is_empty() {
        panic!("{}", message);
    }
}

pub fn require_finite(value: f32, message: &str) {
    if !value.is_finite() {
        panic!("{}", message);
    }
}

pub fn require_finite_non_negative(value: f32, message: &str) {
    require_finite(value, message);
    if value < 0.0 {
        panic!("{}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ElementId;
    use taffy::TaffyTree;

    fn event_context<'a>(
        taffy: &'a TaffyTree<ElementId>,
        focused: &'a mut Option<ElementId>,
    ) -> EventContext<'a> {
        EventContext::new(Bounds::from_xywh(0.0, 0.0, 20.0, 20.0), taffy, focused)
    }

    #[test]
    fn advanced_ui_state_disabled_and_read_only_block_activation() {
        let mut disabled = InteractionState::default();
        disabled.set_disabled(true);
        assert!(!disabled.can_activate());
        assert_eq!(disabled.cursor(), Cursor::NotAllowed);

        let mut read_only = InteractionState::default();
        read_only.set_read_only(true);
        assert!(!read_only.can_activate());
        assert_eq!(read_only.cursor(), Cursor::Default);
    }

    #[test]
    fn advanced_ui_state_hover_updates_redraw_and_cursor() {
        let mut state = InteractionState::default();
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let cx = event_context(&taffy, &mut focused);

        assert!(state.update_hover(
            Bounds::from_xywh(0.0, 0.0, 20.0, 20.0),
            Point::new(2.0, 2.0),
            &cx,
        ));
        assert!(state.hovered());
        assert!(cx.redraw_requested());
        assert_eq!(cx.cursor(), Some(Cursor::Pointer));
    }

    #[test]
    fn advanced_ui_state_release_requires_prior_press() {
        let mut state = InteractionState::default();
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let cx = event_context(&taffy, &mut focused);

        assert!(!state.release_inside(true, &cx).activated);
        assert!(state.press_inside(true, &cx));
        let release = state.release_inside(true, &cx);
        assert!(release.was_pressed);
        assert!(release.activated);
        assert!(!state.pressed());
    }

    #[test]
    fn advanced_ui_state_invalid_is_preserved_for_style_resolution() {
        let mut state = InteractionState::default();
        state.set_invalid(true);

        assert!(state.invalid());
        assert_eq!(
            validation_border_color(state.invalid(), Color::WHITE),
            INVALID_BORDER_COLOR
        );
    }

    #[test]
    fn advanced_ui_state_indexed_press_release_requires_same_index() {
        let mut indexed = IndexedInteractionState::default();
        let state = InteractionState::default();
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let cx = event_context(&taffy, &mut focused);

        assert!(indexed.press(Some(1), &cx, state));
        assert!(!indexed.release(Some(2), &cx, state).activated);

        assert!(indexed.press(Some(1), &cx, state));
        assert!(indexed.release(Some(1), &cx, state).activated);
    }

    #[test]
    #[should_panic(expected = "state label is required")]
    fn advanced_ui_state_validation_rejects_empty_required_text() {
        require_non_empty("  ", "state label is required");
    }
}
