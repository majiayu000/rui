//! Transient notifications stacked in a window corner.
//!
//! A [`Toaster`] is a cloneable handle, like egui-notify's `Toasts` or the
//! `toast()` function sonner pairs with its `<Toaster />`: keep one in a view,
//! clone it into any handler or task on the UI thread and call
//! [`Toaster::show`]. The view places [`Toaster::render`] in its tree, which
//! paints the toasts in an overlay over the page. Each toast closes after its
//! duration, with every timer paused while the pointer is over the stack, or
//! when its close or action button is pressed. Toasts are
//! [`AccessibilityRole::Status`] live regions: assistive technology announces
//! each one when it appears.

use crate::advanced_ui::button::Button;
use crate::advanced_ui::state::require_non_empty;
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::advanced_ui::{Container, column, container, row};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome};
use crate::core::color::Color;
use crate::core::event::KeyEvent;
use crate::core::geometry::{Bounds, Point};
use crate::core::style::{Position, Shadow, Style};
use crate::core::view::{ViewContext, ViewNotifier};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    StableId, dispatch_action_to_children, style_to_taffy,
};
use crate::elements::overlay::OverlaySlot;
use crate::elements::text;
use crate::renderer::Overlay;
use crate::renderer::text::TextMeasureCache;
use std::any::TypeId;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use taffy::prelude::NodeId;

/// How long a toast shows unless given its own duration, as in sonner.
pub const DEFAULT_TOAST_DURATION: Duration = Duration::from_secs(4);
/// How many toasts show at once; showing another closes the oldest.
const DEFAULT_MAX_VISIBLE: usize = 3;
/// Card width at density 1.
const CARD_WIDTH: f32 = 320.0;
/// Width of the variant's accent bar.
const ACCENT_WIDTH: f32 = 4.0;

/// What a toast reports, which sets its accent colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastVariant {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl ToastVariant {
    fn accent(self, theme: &Theme) -> Color {
        match self {
            Self::Info => theme.colors.primary.rest.background,
            Self::Success => theme.colors.success.rest.background,
            Self::Warning => theme.colors.warning,
            Self::Error => theme.state.error,
        }
    }
}

/// The window corner toasts stack in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

impl ToastCorner {
    fn top(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }

    fn left(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }
}

/// Names a shown toast, to close it with [`Toaster::dismiss`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(u64);

#[derive(Clone)]
struct ToastAction {
    label: String,
    handler: Rc<dyn Fn()>,
}

/// One notification: a title, optional description and action button.
#[derive(Clone)]
pub struct Toast {
    title: String,
    description: Option<String>,
    variant: ToastVariant,
    duration: Option<Duration>,
    action: Option<ToastAction>,
}

impl Toast {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        require_non_empty(&title, "toast title must not be empty");
        Self {
            title,
            description: None,
            variant: ToastVariant::default(),
            duration: Some(DEFAULT_TOAST_DURATION),
            action: None,
        }
    }

    pub fn info(title: impl Into<String>) -> Self {
        Self::new(title)
    }

    pub fn success(title: impl Into<String>) -> Self {
        Self::new(title).variant(ToastVariant::Success)
    }

    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(title).variant(ToastVariant::Warning)
    }

    pub fn error(title: impl Into<String>) -> Self {
        Self::new(title).variant(ToastVariant::Error)
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into()).filter(|text| !text.trim().is_empty());
        self
    }

    pub fn variant(mut self, variant: ToastVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Close after `duration` of being shown, not counting time hovered.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Stay until closed.
    pub fn persistent(mut self) -> Self {
        self.duration = None;
        self
    }

    /// A button that runs `handler` and closes the toast.
    pub fn action(mut self, label: impl Into<String>, handler: impl Fn() + 'static) -> Self {
        let label = label.into();
        require_non_empty(&label, "toast action label must not be empty");
        self.action = Some(ToastAction {
            label,
            handler: Rc::new(handler),
        });
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn variant_kind(&self) -> ToastVariant {
        self.variant
    }
}

struct Entry {
    id: ToastId,
    toast: Toast,
    /// Time left to show, as of `since`; `None` for a persistent toast.
    remaining: Option<Duration>,
    since: Instant,
}

struct ToasterState {
    id: ElementId,
    entries: Vec<Entry>,
    next_id: u64,
    paused: bool,
    notifier: Option<ViewNotifier>,
    corner: ToastCorner,
    max_visible: usize,
    theme: Theme,
}

impl ToasterState {
    fn notify(&self) {
        if let Some(notifier) = &self.notifier {
            notifier.notify();
        }
    }

    /// Close the toasts whose time ran out by `now`; whether any did.
    fn expire(&mut self, now: Instant) -> bool {
        if self.paused {
            return false;
        }
        let before = self.entries.len();
        self.entries.retain(|entry| {
            entry
                .remaining
                .is_none_or(|remaining| now.saturating_duration_since(entry.since) < remaining)
        });
        self.entries.len() != before
    }

    /// When the next running timer runs out; none while paused.
    fn next_expiry(&self) -> Option<Instant> {
        if self.paused {
            return None;
        }
        self.entries
            .iter()
            .filter_map(|entry| Some(entry.since + entry.remaining?))
            .min()
    }

    fn set_paused(&mut self, paused: bool, now: Instant) {
        // A card the tree still holds can be under the pointer after its
        // toast closed.
        let paused = paused && !self.entries.is_empty();
        if self.paused == paused {
            return;
        }
        self.paused = paused;
        for entry in &mut self.entries {
            if paused && let Some(remaining) = entry.remaining.as_mut() {
                *remaining = remaining.saturating_sub(now.saturating_duration_since(entry.since));
            }
            entry.since = now;
        }
    }

    /// End the pause once no toast is left: the pointer is over none, and
    /// no move tells the stack so while it is empty.
    fn resume_if_empty(&mut self) {
        if self.entries.is_empty() {
            self.paused = false;
        }
    }
}

/// Shows toasts; see the [module docs](self). Clones share the same toasts.
#[derive(Clone)]
pub struct Toaster {
    state: Rc<RefCell<ToasterState>>,
}

impl Default for Toaster {
    fn default() -> Self {
        Self::new()
    }
}

impl Toaster {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(ToasterState {
                id: ElementId::new(),
                entries: Vec::new(),
                next_id: 0,
                paused: false,
                notifier: None,
                corner: ToastCorner::default(),
                max_visible: DEFAULT_MAX_VISIBLE,
                theme: Theme::default(),
            })),
        }
    }

    /// Give the stack an explicit element id, so its toasts keep their ids
    /// (and focus) across toasters.
    pub fn id(self, id: ElementId) -> Self {
        self.state.borrow_mut().id = id;
        self
    }

    pub fn corner(self, corner: ToastCorner) -> Self {
        self.state.borrow_mut().corner = corner;
        self
    }

    /// How many toasts show at once (at least one); showing another closes
    /// the oldest.
    pub fn max_visible(self, max_visible: usize) -> Self {
        self.state.borrow_mut().max_visible = max_visible.max(1);
        self
    }

    pub fn theme(self, theme: Theme) -> Self {
        self.state.borrow_mut().theme = theme;
        self
    }

    /// Show `toast`, newest nearest the corner, and render the view again.
    pub fn show(&self, toast: Toast) -> ToastId {
        let mut state = self.state.borrow_mut();
        let id = ToastId(state.next_id);
        state.next_id += 1;
        let remaining = toast.duration;
        state.entries.push(Entry {
            id,
            toast,
            remaining,
            since: Instant::now(),
        });
        let overflow = state.entries.len().saturating_sub(state.max_visible);
        state.entries.drain(..overflow);
        state.notify();
        id
    }

    /// Close the toast `id`; whether it was showing.
    pub fn dismiss(&self, id: ToastId) -> bool {
        let mut state = self.state.borrow_mut();
        let before = state.entries.len();
        state.entries.retain(|entry| entry.id != id);
        state.resume_if_empty();
        let dismissed = state.entries.len() != before;
        if dismissed {
            state.notify();
        }
        dismissed
    }

    pub fn dismiss_all(&self) {
        let mut state = self.state.borrow_mut();
        if !state.entries.is_empty() {
            state.entries.clear();
            state.resume_if_empty();
            state.notify();
        }
    }

    /// The toasts showing, oldest first.
    pub fn toasts(&self) -> Vec<ToastId> {
        self.state
            .borrow()
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect()
    }

    /// Whether timers are paused because the pointer is over the stack.
    pub fn is_paused(&self) -> bool {
        self.state.borrow().paused
    }

    /// The stack to place in `cx`'s view: closes toasts whose time ran out
    /// and asks for one frame when the next timer runs out, so the window
    /// sleeps until then.
    pub fn render<V: 'static>(&self, cx: &mut ViewContext<'_, V>) -> ToastStack {
        let next_expiry = {
            let mut state = self.state.borrow_mut();
            state.notifier = Some(cx.notifier());
            state.expire(Instant::now());
            state.next_expiry()
        };
        if let Some(at) = next_expiry {
            cx.request_frame_at(at);
        }
        ToastStack::new(self.clone())
    }

    fn set_paused(&self, paused: bool) {
        let mut state = self.state.borrow_mut();
        let was = state.paused;
        state.set_paused(paused, Instant::now());
        // Timers that start again need the view to ask for their wake.
        if was && !paused {
            state.notify();
        }
    }
}

struct Card {
    slot: OverlaySlot,
    id: ElementId,
    title: String,
    description: Option<String>,
}

/// The toasts of a [`Toaster`], from [`Toaster::render`]. It takes no room in
/// the layout and paints in an overlay in the toaster's corner.
pub struct ToastStack {
    id: StableId,
    toaster: Toaster,
    cards: Vec<Card>,
    /// The card elements, in `cards` order, for [`Element::children`].
    elements: Vec<AnyElement>,
    corner: ToastCorner,
    theme: Theme,
    style: Style,
}

impl ToastStack {
    fn new(toaster: Toaster) -> Self {
        let state = toaster.state.borrow();
        let (stack_id, theme, corner) = (state.id, state.theme, state.corner);
        let (cards, elements) = state
            .entries
            .iter()
            .map(|entry| {
                let id = ElementId::derived(stack_id, entry.id.0, TypeId::of::<Toast>());
                let card = Card {
                    slot: OverlaySlot::default(),
                    id,
                    title: entry.toast.title.clone(),
                    description: entry.toast.description.clone(),
                };
                (
                    card,
                    AnyElement::from(card_element(&toaster, entry, id, theme)),
                )
            })
            .unzip();
        drop(state);
        let mut style = Style::new();
        style.position = Position::Absolute;
        Self {
            id: StableId::explicit(stack_id),
            toaster,
            cards,
            elements,
            corner,
            theme,
            style,
        }
    }

    /// Where each card goes in `window`, in `cards` order; the newest is
    /// nearest the corner.
    fn card_bounds(&self, taffy: &taffy::TaffyTree<ElementId>, window: Bounds) -> Vec<Bounds> {
        let margin = self.theme.control_gap() * 2.0;
        let gap = self.theme.control_gap();
        let mut bounds = vec![Bounds::default(); self.cards.len()];
        let mut edge = if self.corner.top() {
            window.y() + margin
        } else {
            window.max_y() - margin
        };
        for (index, card) in self.cards.iter().enumerate().rev() {
            let size = card.slot.size(taffy).unwrap_or_default();
            let x = if self.corner.left() {
                window.x() + margin
            } else {
                window.max_x() - margin - size.width
            };
            let y = if self.corner.top() {
                edge
            } else {
                edge - size.height
            };
            edge = if self.corner.top() {
                edge + size.height + gap
            } else {
                edge - size.height - gap
            };
            bounds[index] = Bounds::new(Point::new(x.round(), y.round()), size);
        }
        bounds
    }
}

fn card_element(toaster: &Toaster, entry: &Entry, id: ElementId, theme: Theme) -> Container {
    let toast = &entry.toast;
    // No width of its own, so long text wraps instead of pushing the buttons,
    // which then line up at the card's edge in every toast.
    let mut body = column()
        .spacing(theme.control_gap() * 0.5)
        .w(0.0)
        .flex_grow(1.0)
        .child(
            text(toast.title.clone())
                .size(theme.text_size(ControlSize::Medium))
                .semibold()
                .color(theme.colors.text)
                .wrap(),
        );
    if let Some(description) = &toast.description {
        body = body.child(
            text(description.clone())
                .size(theme.text_size(ControlSize::Small))
                .color(theme.colors.text)
                .wrap(),
        );
    }
    let mut content = row()
        .w_full()
        .spacing(theme.control_gap())
        .child(
            container()
                .w(ACCENT_WIDTH)
                .background(toast.variant.accent(&theme))
                .radius(theme.indicator_radius()),
        )
        .child(body);
    let toast_id = entry.id;
    if let Some(action) = &toast.action {
        let (toaster, handler) = (toaster.clone(), Rc::clone(&action.handler));
        content = content.child(
            Button::new(action.label.clone())
                .id(ElementId::derived(id, 1, TypeId::of::<Button>()))
                .outline()
                .small()
                .theme(theme)
                .on_click(move || {
                    handler();
                    toaster.dismiss(toast_id);
                }),
        );
    }
    let toaster = toaster.clone();
    content = content.child(
        Button::new("×")
            .id(ElementId::derived(id, 2, TypeId::of::<Button>()))
            .accessibility_label(format!("Dismiss {}", toast.title))
            .ghost()
            .small()
            .theme(theme)
            .on_click(move || {
                toaster.dismiss(toast_id);
            }),
    );
    container()
        .id(id)
        .w(CARD_WIDTH * theme.density.scale)
        .padding(theme.control_gap() * 1.5)
        .background(theme.colors.surface)
        .border(1.0, theme.colors.border)
        .radius(theme.control_radius())
        .shadow(Shadow::new(
            0.0,
            4.0,
            12.0,
            Color::rgba(0.0, 0.0, 0.0, 0.18),
        ))
        .child(content)
}

impl Element for ToastStack {
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
        for (card, element) in self.cards.iter_mut().zip(self.elements.iter_mut()) {
            card.slot.layout(cx, element);
        }
        cx.new_leaf(style_to_taffy(&self.style))
            .unwrap_or_else(|err| panic!("failed to create toast stack layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if self.cards.is_empty() {
            return;
        }
        let bounds = self.card_bounds(cx.taffy, cx.window());
        let stack = bounds
            .iter()
            .copied()
            .reduce(|a, b| a.union(&b))
            .unwrap_or_default();
        let overlay = Overlay {
            owner: self.id.get(),
            bounds: stack,
            modal: false,
            light_dismiss: false,
            escape: false,
        };
        let id = self.id.get();
        let (cards, elements) = (&self.cards, &mut self.elements);
        cx.paint_overlay(overlay, stack, |cx| {
            cx.register_hit_region(id, stack);
            for ((card, element), bounds) in cards.iter().zip(elements.iter_mut()).zip(bounds) {
                cx.register_accessibility_region(card.id, bounds);
                element.paint(&mut cx.with_bounds(bounds));
            }
        });
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        for element in &mut self.elements {
            element.refresh_text_geometry(text_measurer);
        }
    }

    fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        let mut nodes = Vec::with_capacity(self.cards.len());
        for (card, element) in self.cards.iter().zip(&self.elements) {
            let mut node =
                AccessibilityNode::label_required(card.id, AccessibilityRole::Status, &card.title)?;
            if let Some(description) = &card.description {
                node = node.with_value(description.clone());
            }
            // The card's title and description are already the node's label
            // and value; its text would read them a second time.
            let mut children = element.accessibility_nodes(cx)?;
            crate::elements::element::drop_repeated_text(&node, &mut children);
            nodes.push(node.with_children(children));
        }
        Ok(nodes)
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if self.cards.is_empty() {
            return false;
        }
        let bounds = self.card_bounds(cx.taffy, cx.window());
        let (mut handled, mut inside) = (false, false);
        for (element, bounds) in self.elements.iter_mut().zip(bounds) {
            let mut card_cx = cx.for_overlay(bounds);
            inside |= card_cx.contains_pointer(event.position);
            handled |= element.handle_pointer_event(&mut card_cx, event);
        }
        if matches!(event.kind, PointerEventKind::Move) {
            if inside != self.toaster.is_paused() {
                self.toaster.set_paused(inside);
            }
            return handled;
        }
        handled || inside
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        self.elements
            .iter_mut()
            .any(|element| element.handle_key_event(cx, event))
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        dispatch_action_to_children(&mut self.elements, cx, action)
    }

    fn children(&self) -> &[AnyElement] {
        &self.elements
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        &mut self.elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;
    use crate::core::presenter::Presenter;

    fn entry(state: &mut ToasterState, duration: Option<Duration>, since: Instant) {
        let id = ToastId(state.next_id);
        state.next_id += 1;
        let mut toast = Toast::new("Saved");
        toast.duration = duration;
        state.entries.push(Entry {
            id,
            toast,
            remaining: duration,
            since,
        });
    }

    #[test]
    fn toasts_expire_after_their_duration_and_persistent_ones_stay() {
        let toaster = Toaster::new();
        let start = Instant::now();
        let mut state = toaster.state.borrow_mut();
        entry(&mut state, Some(Duration::from_secs(4)), start);
        entry(&mut state, None, start);
        assert!(!state.expire(start + Duration::from_secs(3)));
        assert!(state.expire(start + Duration::from_secs(4)));
        assert_eq!(state.entries.len(), 1);
        assert!(state.entries[0].remaining.is_none());
    }

    #[test]
    fn hovering_pauses_the_countdown() {
        let toaster = Toaster::new();
        let start = Instant::now();
        let mut state = toaster.state.borrow_mut();
        entry(&mut state, Some(Duration::from_secs(4)), start);
        state.set_paused(true, start + Duration::from_secs(3));
        assert!(!state.expire(start + Duration::from_secs(60)));
        state.set_paused(false, start + Duration::from_secs(60));
        assert!(!state.expire(start + Duration::from_millis(60_900)));
        assert!(state.expire(start + Duration::from_secs(61)));
    }

    #[test]
    fn closing_the_last_toast_ends_the_pause() {
        let toaster = Toaster::new();
        let first = toaster.show(Toast::new("one"));
        toaster.show(Toast::new("two"));
        let viewport = Size::new(480.0, 320.0);
        let page = container()
            .w_full()
            .h_full()
            .child(ToastStack::new(toaster.clone()));
        let mut presenter = Presenter::with_root(viewport, page);
        presenter.layout(viewport).expect("layout");
        presenter.paint();
        let stack = presenter.scene().overlays()[0].bounds;
        let over_a_card = PointerEvent {
            kind: PointerEventKind::Move,
            position: Point::new(stack.x() + 8.0, stack.y() + 8.0),
            button: None,
        };
        presenter.dispatch_pointer_event(&over_a_card);
        assert!(toaster.is_paused());

        assert!(toaster.dismiss(first));
        assert!(toaster.is_paused(), "the pointer may be over the other");
        toaster.dismiss_all();
        assert!(!toaster.is_paused());
        // The tree holds the cards until the view renders again, and the
        // pointer moves on over them in the frame of the click that closed one.
        presenter.dispatch_pointer_event(&over_a_card);
        assert!(!toaster.is_paused());
    }

    #[test]
    fn showing_past_the_limit_closes_the_oldest() {
        let toaster = Toaster::new().max_visible(2);
        let first = toaster.show(Toast::new("one"));
        let second = toaster.show(Toast::new("two"));
        let third = toaster.show(Toast::new("three"));
        assert_eq!(toaster.toasts(), vec![second, third]);
        assert!(!toaster.dismiss(first));
        assert!(toaster.dismiss(second));
        assert_eq!(toaster.toasts(), vec![third]);
    }
}
