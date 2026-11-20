//! Selecting and copying static text.
//!
//! A [`SelectionScope`] makes every [`Text`] and [`StyledText`] inside it
//! selectable as one document, as a browser page or a chat reply is: a drag
//! selects from where it pressed to where it is, across wrapped lines and
//! from one text to the next in the order they paint. A double click selects
//! a word, a triple click a paragraph (the text between line breaks), and a
//! shift-click extends the selection. While the scope has focus, which a press
//! on it gives it, the copy action (Cmd/Ctrl+C) puts the selected text on the
//! clipboard, separate texts joined by line breaks, and select all (Cmd/Ctrl+A)
//! selects all of its text. A click that does not drag still reaches links.
//!
//! The text inside stays as cheap as any other: it lays out and paints
//! unchanged frames from the previous one, and only tells the scope where its
//! clusters are, from the shaping the renderer does, while there is a
//! selection to hit-test or highlight. The highlight is the theme's selection
//! color over the selected clusters.
//!
//! The selection is kept across rebuilds while the text it starts and ends in
//! still begins the same way, so text streamed onto the end of a reply leaves a
//! selection in its earlier blocks where it was.
//!
//! [`Text`]: crate::elements::Text
//! [`StyledText`]: crate::elements::StyledText

mod leaf;

pub(crate) use leaf::{LeafFont, LeafLayout, SelectableLeaf};

use crate::advanced_ui::Theme;
use crate::core::ElementId;
use crate::core::SharedStr;
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::color::Color;
use crate::core::drag::DRAG_THRESHOLD;
use crate::core::event::{Cursor, MouseButton};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    StableId, dispatch_action_to_children,
};
use crate::elements::{StyledText, Text};
use crate::renderer::{Primitive, Scene};
use std::any::Any;
use std::ops::Range;
use std::time::{Duration, Instant};
use taffy::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

/// Note where `element` is, when it is selectable text, in the scene a
/// selection scope collects its text with.
pub(crate) fn collect_leaf(element: &mut dyn Element, cx: &mut PaintContext) {
    // An overlay opened from inside the scope, such as a popover, is a
    // document of its own.
    if cx.scene.in_overlay() {
        return;
    }
    let (bounds, clip) = (cx.bounds(), cx.scene.current_clip());
    let element = element as &mut dyn Any;
    let leaf = if let Some(text) = element.downcast_mut::<Text>() {
        text.selectable_leaf(bounds, clip)
    } else if let Some(text) = element.downcast_mut::<StyledText>() {
        text.selectable_leaf(bounds, clip)
    } else {
        None
    };
    if let (Some(leaf), Some(leaves)) = (leaf, &mut cx.scene.selection) {
        leaves.push(leaf);
    }
}

/// Presses this close together in time and place count as one multi-click.
const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);

/// A place in a scope's text: a byte of the `leaf`th text it painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TextPoint {
    leaf: usize,
    offset: usize,
}

/// What a drag extends the selection by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Granularity {
    Character,
    Word,
    Paragraph,
}

/// A press that may become a drag.
#[derive(Debug, Clone, Copy)]
struct Press {
    at: Point,
    granularity: Granularity,
    /// The unit the press selected, which a drag keeps selected.
    origin: (TextPoint, TextPoint),
    dragged: bool,
}

/// Where the selection starts and ends, with the text of those two leaves
/// when it was made, to tell whether a rebuild left them in place.
#[derive(Debug, Clone)]
struct Selection {
    anchor: TextPoint,
    focus: TextPoint,
    anchor_text: SharedStr,
    focus_text: SharedStr,
}

impl Selection {
    fn ordered(&self) -> (TextPoint, TextPoint) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }

    fn is_empty(&self) -> bool {
        self.anchor == self.focus
    }
}

/// What a scope keeps across rebuilds.
#[derive(Debug, Default)]
struct ScopeState {
    selection: Option<Selection>,
    /// The texts painted in the scope when it last collected them.
    leaves: Vec<SelectableLeaf>,
    press: Option<Press>,
    /// The last press: when and where, and how many presses it ended.
    last_click: Option<(Instant, Point, u32)>,
}

/// An element whose text can be selected and copied. See the
/// [module docs](self).
pub struct SelectionScope {
    id: StableId,
    child: AnyElement,
    color: Color,
    state: ScopeState,
}

/// Make the text in `child` selectable as one document.
pub fn selection_scope(child: impl Into<AnyElement>) -> SelectionScope {
    SelectionScope {
        id: StableId::new(),
        child: child.into(),
        color: Theme::default().state.selection,
        state: ScopeState::default(),
    }
}

impl SelectionScope {
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    /// Highlight the selection with `theme`'s selection color.
    pub fn theme(self, theme: &Theme) -> Self {
        self.selection_color(theme.state.selection)
    }

    /// Highlight the selection with `color`, drawn over the text.
    pub fn selection_color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    /// The selected text, as copying would put it on the clipboard: `None`
    /// when nothing is selected.
    ///
    /// Texts side by side, as the cells of a table row are, are joined by a
    /// tab; a text below the one before it starts a new line.
    pub fn selected_text(&self) -> Option<String> {
        let mut text: Option<String> = None;
        let mut previous: Option<Bounds> = None;
        for (leaf, range) in self.selected_ranges() {
            let piece = leaf.content.get(range)?;
            match (&mut text, previous) {
                (Some(text), Some(previous)) => {
                    text.push(if beside(previous, leaf.bounds) {
                        '\t'
                    } else {
                        '\n'
                    });
                    text.push_str(piece);
                }
                _ => text = Some(piece.to_string()),
            }
            previous = Some(leaf.bounds);
        }
        text
    }

    /// Whether `point` is over an overlay opened from inside the scope, which
    /// the scope's selection leaves alone.
    fn over_own_overlay(&self, cx: &EventContext, point: Point) -> bool {
        cx.overlays()
            .iter()
            .any(|overlay| overlay.bounds.contains(point) && self.child.contains_id(overlay.owner))
    }

    /// Each text the selection covers, with the bytes of it selected.
    fn selected_ranges(&self) -> impl Iterator<Item = (&SelectableLeaf, Range<usize>)> {
        let selection = self.state.selection.as_ref().filter(|s| !s.is_empty());
        let (start, end) = selection.map_or(
            (
                TextPoint { leaf: 1, offset: 0 },
                TextPoint { leaf: 0, offset: 0 },
            ),
            Selection::ordered,
        );
        let leaves = self
            .state
            .leaves
            .get(start.leaf..=end.leaf)
            .unwrap_or_default();
        leaves.iter().enumerate().map(move |(index, leaf)| {
            let index = start.leaf + index;
            let from = if index == start.leaf { start.offset } else { 0 };
            let to = if index == end.leaf {
                end.offset
            } else {
                leaf.content.len()
            };
            (leaf, from..to)
        })
    }

    /// Carry the selection from the scope `previous` this one is rebuilt in
    /// place of.
    pub(crate) fn reconcile_selection(&mut self, previous: &mut SelectionScope) {
        if self.id.is_explicit() == previous.id.is_explicit()
            && (!self.id.is_explicit() || self.id == previous.id)
        {
            self.state = std::mem::take(&mut previous.state);
        }
    }

    /// Paint the child again, without replay and into a scene of its own,
    /// for where its texts are now.
    fn collect(&mut self, taffy: &TaffyTree<ElementId>, bounds: Bounds) {
        let mut scene = Scene::new();
        scene.selection = Some(Vec::new());
        self.child
            .paint(&mut PaintContext::without_replay(&mut scene, bounds, taffy));
        let mut leaves = scene.selection.take().unwrap_or_default();
        for (leaf, previous) in leaves.iter_mut().zip(self.state.leaves.iter_mut()) {
            leaf.adopt_geometry(previous);
        }
        self.state.leaves = leaves;
        self.keep_valid_selection();
    }

    /// Drop the selection when a rebuild changed the text it starts or ends
    /// in before the place it does.
    fn keep_valid_selection(&mut self) {
        let leaves = &self.state.leaves;
        let holds = |point: TextPoint, text: &SharedStr| {
            leaves.get(point.leaf).is_some_and(|leaf| {
                let content = leaf.content.as_str();
                content.is_char_boundary(point.offset)
                    && content.get(..point.offset) == text.get(..point.offset)
            })
        };
        if let Some(selection) = &mut self.state.selection {
            if holds(selection.anchor, &selection.anchor_text)
                && holds(selection.focus, &selection.focus_text)
            {
                selection.anchor_text = leaves[selection.anchor.leaf].content.clone();
                selection.focus_text = leaves[selection.focus.leaf].content.clone();
            } else {
                self.state.selection = None;
            }
        }
    }

    /// The place in the scope's text nearest `point`: in the text under it,
    /// else the nearest one, up or down first.
    fn point_at(&self, point: Point) -> Option<TextPoint> {
        let leaves = &self.state.leaves;
        let leaf = leaves
            .iter()
            .position(|leaf| leaf.contains(point))
            .or_else(|| {
                (0..leaves.len()).min_by(|a, b| {
                    let (a, b) = (leaves[*a].distance(point), leaves[*b].distance(point));
                    a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
                })
            })?;
        Some(TextPoint {
            leaf,
            offset: leaves[leaf].geometry().offset_at(point),
        })
    }

    /// The word or paragraph around `at`, or just `at`.
    fn unit_at(&self, at: TextPoint, granularity: Granularity) -> (TextPoint, TextPoint) {
        let content = self.state.leaves[at.leaf].content.as_str();
        let range = match granularity {
            Granularity::Character => at.offset..at.offset,
            Granularity::Word => word_at(content, at.offset),
            Granularity::Paragraph => paragraph_at(content, at.offset),
        };
        (
            TextPoint {
                leaf: at.leaf,
                offset: range.start,
            },
            TextPoint {
                leaf: at.leaf,
                offset: range.end,
            },
        )
    }

    fn select(&mut self, anchor: TextPoint, focus: TextPoint) {
        let text = |point: TextPoint| self.state.leaves[point.leaf].content.clone();
        self.state.selection = Some(Selection {
            anchor,
            focus,
            anchor_text: text(anchor),
            focus_text: text(focus),
        });
    }

    /// Whether the unit `press` selected is still in the scope's text: a
    /// rebuild while the button is down can take the text it was in away.
    fn has_origin(&self, press: &Press) -> bool {
        let in_text = |point: TextPoint| {
            let leaf = self.state.leaves.get(point.leaf);
            leaf.is_some_and(|leaf| leaf.content.is_char_boundary(point.offset))
        };
        in_text(press.origin.0) && in_text(press.origin.1)
    }

    /// Select from the press's unit to the unit at `at`, keeping both.
    fn extend_press(&mut self, press: Press, at: TextPoint) {
        let (start, end) = self.unit_at(at, press.granularity);
        let (origin_start, origin_end) = press.origin;
        if start < origin_start {
            self.select(origin_end, start);
        } else {
            self.select(origin_start, end.max(origin_end));
        }
    }

    fn select_all(&mut self) -> bool {
        let Some(last) = self.state.leaves.len().checked_sub(1) else {
            return false;
        };
        let end = self.state.leaves[last].content.len();
        self.select(
            TextPoint { leaf: 0, offset: 0 },
            TextPoint {
                leaf: last,
                offset: end,
            },
        );
        true
    }

    /// How many presses `at` ends, counting the ones just before it near it.
    fn count_click(&mut self, at: Point) -> u32 {
        let now = Instant::now();
        let count = match self.state.last_click {
            Some((time, place, count))
                if now.duration_since(time) <= MULTI_CLICK_INTERVAL
                    && (place.x - at.x).abs() <= DRAG_THRESHOLD
                    && (place.y - at.y).abs() <= DRAG_THRESHOLD =>
            {
                count % 3 + 1
            }
            _ => 1,
        };
        self.state.last_click = Some((now, at, count));
        count
    }

    /// Start selecting at a press, the texts collected.
    fn press(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let Some(at) = self.point_at(event.position) else {
            return false;
        };
        let granularity = match self.count_click(event.position) {
            1 => Granularity::Character,
            2 => Granularity::Word,
            _ => Granularity::Paragraph,
        };
        let extend = cx.modifiers().shift && granularity == Granularity::Character;
        let origin = match (&self.state.selection, extend) {
            (Some(selection), true) => (selection.anchor, selection.anchor),
            _ => self.unit_at(at, granularity),
        };
        let press = Press {
            at: event.position,
            granularity,
            origin,
            // A word or paragraph is selected at once: the release is no click.
            dragged: granularity != Granularity::Character,
        };
        if extend {
            self.select(origin.0, at);
        } else {
            self.select(origin.0, origin.1);
        }
        self.state.press = Some(press);
        cx.request_focus(Some(self.id.get()));
        cx.request_redraw();
        true
    }

    fn drag(&mut self, cx: &mut EventContext, event: &PointerEvent) {
        let Some(mut press) = self.state.press else {
            return;
        };
        let moved = (event.position.x - press.at.x).abs() > DRAG_THRESHOLD
            || (event.position.y - press.at.y).abs() > DRAG_THRESHOLD;
        press.dragged |= moved;
        self.state.press = Some(press);
        // Without the text it started in, the drag has nothing to select
        // from; its release is still not a click.
        if !press.dragged || !self.has_origin(&press) {
            return;
        }
        if let Some(at) = self.point_at(event.position) {
            self.extend_press(press, at);
            cx.request_redraw();
        }
    }

    fn paint_highlight(&self, cx: &mut PaintContext) {
        for (leaf, range) in self.selected_ranges() {
            for rect in leaf.geometry().rects(range) {
                let rect = match leaf.clip {
                    Some(clip) => match rect.intersection(&clip) {
                        Some(rect) => rect,
                        None => continue,
                    },
                    None => rect,
                };
                cx.paint(Primitive::Quad {
                    bounds: rect,
                    background: self.color.to_rgba(),
                    border_color: Color::TRANSPARENT.to_rgba(),
                    border_widths: Edges::all(0.0),
                    corner_radii: Corners::all(0.0),
                });
            }
        }
    }
}

impl Element for SelectionScope {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.get())
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        Some(&mut self.id)
    }

    fn style(&self) -> &Style {
        self.child.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.child.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        self.child.paint(cx);
        if self.state.selection.as_ref().is_some_and(|s| !s.is_empty()) {
            self.collect(cx.taffy, bounds);
            self.paint_highlight(cx);
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let inside = cx.contains_pointer(event.position);
        match event.kind {
            PointerEventKind::Move => {
                if self.state.press.is_none() && self.over_own_overlay(cx, event.position) {
                    return self.child.handle_pointer_event(cx, event);
                }
                if self.state.press.is_some() {
                    cx.set_cursor(Cursor::Text);
                    self.drag(cx, event);
                    return true;
                }
                if inside {
                    // Set first, so a link inside shows its own cursor.
                    cx.set_cursor(Cursor::Text);
                }
                self.child.handle_pointer_event(cx, event)
            }
            PointerEventKind::Down => {
                let primary = matches!(event.button, None | Some(MouseButton::Left));
                if self.over_own_overlay(cx, event.position) {
                    // Its content is not the scope's: leave the selection be,
                    // so a menu opened on it can still copy it.
                    return self.child.handle_pointer_event(cx, event);
                }
                // The owners of light-dismiss overlays see a press before it is
                // dispatched, as one outside them: the scope it lands in only
                // passes it on.
                if cx
                    .dismiss_target()
                    .is_some_and(|target| self.contains_id(target))
                {
                    return self.child.handle_pointer_event(cx, event);
                }
                if !inside || !primary {
                    if !inside && self.state.selection.take().is_some() {
                        cx.request_redraw();
                    }
                    return self.child.handle_pointer_event(cx, event);
                }
                let handled = self.child.handle_pointer_event(cx, event);
                self.collect(cx.taffy, cx.layout_bounds());
                let on_text = self
                    .state
                    .leaves
                    .iter()
                    .any(|leaf| leaf.contains(event.position));
                if handled && !on_text {
                    // A control in the scope took the press.
                    return true;
                }
                self.press(cx, event) || handled
            }
            PointerEventKind::Up => {
                let Some(press) = self.state.press.take() else {
                    return self.child.handle_pointer_event(cx, event);
                };
                if !press.dragged {
                    // A click: links and controls under it still see it.
                    self.child.handle_pointer_event(cx, event);
                }
                true
            }
        }
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        let outcome =
            dispatch_action_to_children(std::slice::from_mut(&mut self.child), cx, action);
        if outcome.is_handled() || !cx.is_focused(Some(self.id.get())) {
            return outcome;
        }
        match action {
            ActionId::Standard(StandardAction::Copy) => {
                let Some(text) = self.selected_text() else {
                    return ActionOutcome::Ignored;
                };
                let Some(mut clipboard) = cx.clipboard() else {
                    log::error!("selection copy ran without a clipboard in its event context");
                    return ActionOutcome::Ignored;
                };
                if let Err(err) = clipboard.write_text(&text) {
                    log::error!("selection copy failed: {err}");
                }
                ActionOutcome::handled("selection_scope")
            }
            ActionId::Standard(StandardAction::SelectAll) => {
                if self.select_all() {
                    cx.request_redraw();
                    ActionOutcome::handled("selection_scope")
                } else {
                    ActionOutcome::Ignored
                }
            }
            ActionId::Standard(StandardAction::Cancel) if self.state.selection.take().is_some() => {
                cx.request_redraw();
                ActionOutcome::handled("selection_scope")
            }
            _ => ActionOutcome::Ignored,
        }
    }

    fn children(&self) -> &[AnyElement] {
        std::slice::from_ref(&self.child)
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        std::slice::from_mut(&mut self.child)
    }
}

/// Whether a text at `next` sits to the right of one at `previous` on the
/// same row, as the next cell of a table row does.
fn beside(previous: Bounds, next: Bounds) -> bool {
    let previous_bottom = previous.y() + previous.height();
    let next_bottom = next.y() + next.height();
    next.y() < previous_bottom
        && previous.y() < next_bottom
        && next.x() >= previous.x() + previous.width() - 0.5
}

/// The word, or run of spaces or punctuation, `offset` is in or ends.
fn word_at(content: &str, offset: usize) -> Range<usize> {
    let mut last = offset..offset;
    for (start, word) in content.split_word_bound_indices() {
        let end = start + word.len();
        if offset < end {
            return start..end;
        }
        last = start..end;
    }
    last
}

/// The line of `content` between line breaks that `offset` is on.
fn paragraph_at(content: &str, offset: usize) -> Range<usize> {
    let start = content[..offset].rfind('\n').map_or(0, |at| at + 1);
    let end = content[offset..]
        .find('\n')
        .map_or(content.len(), |at| offset + at);
    start..end
}
