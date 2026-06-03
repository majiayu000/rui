use crate::advanced_ui::state::require_non_empty;
use crate::advanced_ui::tokens::Theme;
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::color::Color;
use crate::core::event::ScrollEvent;
use crate::core::style::Style;
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::elements::{Div, div};
use taffy::prelude::NodeId;

pub struct TabPanel {
    id: ElementId,
    value: String,
    accessibility_label: Option<String>,
    inner: Div,
    theme: Theme,
    custom_background: bool,
    custom_border: bool,
    custom_radius: bool,
}

impl TabPanel {
    pub fn new(value: impl Into<String>, child: impl Into<AnyElement>) -> Self {
        let value = value.into();
        require_non_empty(&value, "tab panel value must not be empty");
        let id = ElementId::new();
        let theme = Theme::default();
        Self {
            id,
            value,
            accessibility_label: None,
            inner: base_tab_panel_div(id, theme).child(child),
            theme,
            custom_background: false,
            custom_border: false,
            custom_radius: false,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self.inner = self.inner.id(id);
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "tab panel accessibility label must not be empty");
        self.accessibility_label = Some(label);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        if !self.custom_background {
            self.inner = self.inner.bg(theme.colors.surface);
        }
        if !self.custom_border {
            self.inner = self.inner.border(1.0, theme.colors.border);
        }
        if !self.custom_radius {
            self.inner = self.inner.rounded(theme.control_radius());
        }
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.inner = self.inner.p(padding);
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.custom_background = true;
        self.inner = self.inner.bg(color);
        self
    }

    pub fn border(mut self, width: f32, color: impl Into<Color>) -> Self {
        self.custom_border = true;
        self.inner = self.inner.border(width, color);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.custom_radius = true;
        self.inner = self.inner.rounded(radius);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.inner = self.inner.flex_grow(grow);
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub(crate) fn accessibility_nodes_with_label(
        &self,
        cx: &AccessibilityContext,
        fallback_label: &str,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        let label = self
            .accessibility_label
            .as_deref()
            .unwrap_or(fallback_label);
        let children = Element::accessibility_nodes(&self.inner, cx)?;
        Ok(vec![
            AccessibilityNode::label_required(self.id, AccessibilityRole::TabPanel, label)?
                .value_required(&self.value)?
                .with_focused(cx.a11y_has_focus(self.id))
                .with_children(children),
        ])
    }
}

impl Element for TabPanel {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        Element::style(&self.inner)
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let label = self.accessibility_label.as_deref().unwrap_or(&self.value);
        Ok(Some(
            AccessibilityNode::label_required(self.id, AccessibilityRole::TabPanel, label)?
                .value_required(&self.value)?
                .with_focused(cx.a11y_has_focus(self.id)),
        ))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        self.inner.handle_pointer_event(cx, event)
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        self.inner.handle_key_event(cx, event)
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        self.inner.handle_scroll_event(cx, event)
    }

    fn children(&self) -> &[AnyElement] {
        Element::children(&self.inner)
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.inner.contains_id(id)
    }
}

fn base_tab_panel_div(id: ElementId, theme: Theme) -> Div {
    div()
        .id(id)
        .flex_col()
        .p(12.0)
        .bg(theme.colors.surface)
        .border(1.0, theme.colors.border)
        .rounded(theme.control_radius())
}
