//! The inspector: point at an element to read what the last frame knew about
//! it, beside a tree of every element painted.

use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityError, AccessibilityTree};
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::{Background, Corners, Display, FlexDirection, Overflow, Style};
use crate::elements::element::{AnyElement, Element, PaintContext, PointerEvent, PointerEventKind};
use crate::elements::text::TextAlign;
use crate::renderer::{Primitive, Scene};
use std::cell::RefCell;
use std::fmt::Write as _;
use taffy::prelude::TaffyTree;

const PANEL_WIDTH: f32 = 440.0;
const MARGIN: f32 = 8.0;
const PADDING: f32 = 8.0;
const FONT_SIZE: f32 = 11.0;
const LINE_HEIGHT: f32 = 15.0;
const DETAIL_LINES: usize = 6;
const TREE_LINES: usize = 12;
/// What fits in the panel's width at the font size.
const LINE_CHARS: usize = 64;
/// Deeper elements in the tree are indented no further.
const MAX_INDENT: usize = 12;
const FONT_FAMILY: &str = "SF Mono";
const HOVERED: Rgba = Rgba::new(0.3, 0.65, 1.0, 0.9);
const HOVERED_FILL: Rgba = Rgba::new(0.3, 0.65, 1.0, 0.16);
const SELECTED: Rgba = Rgba::new(1.0, 0.6, 0.1, 0.95);

/// How the last frame painted an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintState {
    /// Painted again, and recorded so the next frame can replay it.
    Repainted,
    /// Replayed from the frame before.
    Replayed,
    /// Inside a subtree that was replayed, so not visited.
    Kept,
    /// Out of view, so left out with its subtree.
    Skipped,
    /// Painted again and not recorded: it, or something in it, cannot replay.
    Painted,
}

impl PaintState {
    fn label(self) -> &'static str {
        match self {
            Self::Repainted => "repainted",
            Self::Replayed => "replayed",
            Self::Kept => "kept",
            Self::Skipped => "skipped",
            Self::Painted => "painted",
        }
    }
}

/// One element of the last frame, as the inspector saw it.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectedElement {
    /// The element's stable identity.
    pub id: ElementId,
    /// The application set the id, rather than layout deriving it from the
    /// element's place in the tree.
    pub explicit_id: bool,
    /// The element's type, without module paths.
    pub type_name: String,
    /// Index of the parent in [`Inspector::elements`]; `None` for the root.
    pub parent: Option<usize>,
    pub depth: usize,
    /// Where the element was painted, which is its layout bounds moved by any
    /// scrolling above it.
    pub bounds: Bounds,
    /// The style properties set to something other than their default.
    pub style: String,
    pub paint: PaintState,
    /// Role and label of the element's accessibility node, if it has one.
    pub accessibility: Option<String>,
}

/// An element with the frame and index of the paint record it left.
type Visited = (InspectedElement, Option<(u64, u32)>);

/// Collects the elements a paint without replay visits, in paint order.
#[derive(Default)]
pub(crate) struct Inspection {
    elements: RefCell<Vec<Visited>>,
    /// The elements being painted, outermost first.
    open: RefCell<Vec<usize>>,
}

impl Inspection {
    pub(crate) fn enter(&self, element: &AnyElement, bounds: Bounds) {
        let id = element
            .stable_id()
            .expect("inspector: an element was painted before it was laid out");
        self.push(
            id,
            element.type_name(),
            element.style(),
            bounds,
            element.paint_record_slot(),
        );
    }

    fn enter_root(&self, root: &dyn Element, bounds: Bounds) {
        // The root is laid out outside any parent, so its identity is fixed.
        let id = root
            .id()
            .filter(|id| !id.is_ephemeral())
            .unwrap_or(ElementId::ROOT);
        self.push(id, root.type_name(), root.style(), bounds, None);
    }

    fn push(
        &self,
        id: ElementId,
        type_name: &str,
        style: &Style,
        bounds: Bounds,
        slot: Option<(u64, u32)>,
    ) {
        let mut open = self.open.borrow_mut();
        let mut elements = self.elements.borrow_mut();
        let element = InspectedElement {
            id,
            explicit_id: !id.is_derived(),
            type_name: short_type_name(type_name),
            parent: open.last().copied(),
            depth: open.len(),
            bounds,
            style: style_summary(style),
            paint: PaintState::Painted,
            accessibility: None,
        };
        open.push(elements.len());
        elements.push((element, slot));
    }

    pub(crate) fn exit(&self) {
        self.open.borrow_mut().pop();
    }

    /// The elements with how `scene`, the frame just painted, painted them.
    fn finish(
        self,
        scene: &Scene,
        accessibility: &Result<AccessibilityTree, AccessibilityError>,
    ) -> Vec<InspectedElement> {
        let entries = self.elements.into_inner();
        let mut elements: Vec<InspectedElement> = Vec::with_capacity(entries.len());
        for (mut element, slot) in entries {
            let record = slot
                .filter(|(frame, _)| *frame == scene.frame())
                .and_then(|(frame, index)| scene.paint_record(frame, index));
            element.paint = match record {
                Some(record) if !record.replayed => PaintState::Repainted,
                Some(record) if record.painted => PaintState::Replayed,
                Some(_) => PaintState::Skipped,
                // Replay passes the subtree by, so only its root has a record.
                None => match element.parent.map(|parent| elements[parent].paint) {
                    Some(PaintState::Replayed | PaintState::Kept) => PaintState::Kept,
                    Some(PaintState::Skipped) => PaintState::Skipped,
                    _ => PaintState::Painted,
                },
            };
            element.accessibility = match accessibility {
                Ok(tree) => tree.find(element.id).map(|node| match node.a11y_label() {
                    Some(label) => format!("{:?} {label:?}", node.a11y_role()),
                    None => format!("{:?}", node.a11y_role()),
                }),
                Err(err) => Some(format!("tree failed: {err:?}")),
            };
            elements.push(element);
        }
        elements
    }
}

/// Per-window inspector state, alive only while the inspector is on.
///
/// Pointer presses and moves go to the inspector instead of the app: a move
/// hovers the element under the pointer, a press pins it, and pressing the
/// pinned element again unpins it.
pub struct Inspector {
    elements: Vec<InspectedElement>,
    hovered: Option<ElementId>,
    selected: Option<ElementId>,
    /// Takes the paint the inspector walks the tree with.
    scratch: Scene,
}

impl std::fmt::Debug for Inspector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inspector")
            .field("elements", &self.elements.len())
            .field("hovered", &self.hovered)
            .field("selected", &self.selected)
            .finish()
    }
}

impl Inspector {
    pub(crate) fn new() -> Self {
        Self {
            elements: Vec::new(),
            hovered: None,
            selected: None,
            scratch: Scene::new(),
        }
    }

    /// Every element of the last frame in paint order, each after its parent.
    pub fn elements(&self) -> &[InspectedElement] {
        &self.elements
    }

    /// The element painted last, so on top, at `point`.
    pub fn element_at(&self, point: Point) -> Option<&InspectedElement> {
        self.elements
            .iter()
            .rev()
            .find(|element| element.bounds.contains(point))
    }

    pub fn find(&self, id: ElementId) -> Option<&InspectedElement> {
        self.elements.iter().find(|element| element.id == id)
    }

    /// The element under the pointer.
    pub fn hovered(&self) -> Option<&InspectedElement> {
        self.find(self.hovered?)
    }

    /// The element pinned with a press.
    pub fn selected(&self) -> Option<&InspectedElement> {
        self.find(self.selected?)
    }

    /// Take a pointer event the app would have had. Returns whether the
    /// overlay changed.
    pub(crate) fn pointer(&mut self, event: &PointerEvent) -> bool {
        let at = self.element_at(event.position).map(|element| element.id);
        match event.kind {
            PointerEventKind::Move => std::mem::replace(&mut self.hovered, at) != at,
            PointerEventKind::Down => {
                self.hovered = at;
                self.selected = if self.selected == at { None } else { at };
                true
            }
            PointerEventKind::Up => false,
        }
    }

    /// Walk the tree `scene` was just painted from by painting it again
    /// without replay, so every element is visited.
    pub(crate) fn inspect<E: Element>(
        &mut self,
        root: &mut E,
        taffy: &TaffyTree<ElementId>,
        bounds: Bounds,
        scene: &Scene,
        accessibility: &Result<AccessibilityTree, AccessibilityError>,
    ) {
        let inspection = Inspection::default();
        inspection.enter_root(&*root, bounds);
        self.scratch.clear();
        root.paint(&mut PaintContext::inspecting(
            &mut self.scratch,
            bounds,
            taffy,
            &inspection,
        ));
        inspection.exit();
        self.elements = inspection.finish(scene, accessibility);
    }

    /// Outline the hovered and pinned elements and draw the panel.
    pub(crate) fn paint(&self, scene: &mut Scene, viewport: Size) {
        let hovered = self.hovered();
        let selected = self.selected();
        if let Some(element) = hovered {
            scene.insert(outline(element.bounds, HOVERED_FILL, HOVERED, 1.0));
        }
        if let Some(element) = selected {
            scene.insert(outline(element.bounds, Rgba::TRANSPARENT, SELECTED, 2.0));
        }
        let target = selected.or(hovered);
        let lines = self.lines(target);
        let height = PADDING * 2.0 + LINE_HEIGHT * lines.len() as f32;
        // Out of the way of what is inspected.
        let low = target.is_some_and(|element| {
            element.bounds.origin.y + element.bounds.size.height / 2.0 > viewport.height / 2.0
        });
        let y = if low {
            MARGIN
        } else {
            (viewport.height - height - MARGIN).max(0.0)
        };
        let panel = Bounds::from_xywh(MARGIN, y, PANEL_WIDTH, height);
        scene.insert(Primitive::Quad {
            bounds: panel,
            background: Rgba::new(0.05, 0.06, 0.08, 0.9),
            border_color: Rgba::new(1.0, 1.0, 1.0, 0.18),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::all(6.0),
        });
        for (row, line) in lines.into_iter().enumerate() {
            scene.insert(Primitive::Text {
                bounds: Bounds::from_xywh(
                    panel.origin.x + PADDING,
                    panel.origin.y + PADDING + LINE_HEIGHT * row as f32,
                    PANEL_WIDTH - PADDING * 2.0,
                    LINE_HEIGHT,
                ),
                content: line,
                color: Rgba::new(0.88, 0.94, 1.0, 1.0),
                font_size: FONT_SIZE,
                font_weight: 400,
                font_family: Some(FONT_FAMILY.to_string()),
                line_height: LINE_HEIGHT,
                align: TextAlign::Left,
            });
        }
    }

    /// The details of `target`, then the part of the tree around it.
    fn lines(&self, target: Option<&InspectedElement>) -> Vec<String> {
        let mut lines = Vec::with_capacity(DETAIL_LINES + TREE_LINES);
        lines.push(format!(
            "rui inspector  {} elements  {}",
            self.elements.len(),
            if self.selected.is_some() {
                "pinned: press it to unpin"
            } else {
                "press to pin"
            }
        ));
        match target {
            Some(element) => {
                let bounds = element.bounds;
                lines.push(element.type_name.clone());
                lines.push(format!(
                    "id {:016x} ({})",
                    element.id.0,
                    if element.explicit_id {
                        "explicit"
                    } else {
                        "derived"
                    }
                ));
                lines.push(format!(
                    "bounds {:.1},{:.1}  {:.1} x {:.1}",
                    bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height
                ));
                lines.push(format!("style {}", element.style));
                lines.push(format!(
                    "paint {}  a11y {}",
                    element.paint.label(),
                    element.accessibility.as_deref().unwrap_or("none")
                ));
            }
            None => lines.push(String::from("point at an element")),
        }
        lines.resize(DETAIL_LINES, String::new());
        let at = target
            .and_then(|target| {
                self.elements
                    .iter()
                    .position(|element| element.id == target.id)
            })
            .unwrap_or(0);
        let start = at
            .saturating_sub(TREE_LINES / 2)
            .min(self.elements.len().saturating_sub(TREE_LINES));
        for (index, element) in self
            .elements
            .iter()
            .enumerate()
            .skip(start)
            .take(TREE_LINES)
        {
            let mut line = String::from(if target.is_some() && index == at {
                "> "
            } else {
                "  "
            });
            for _ in 0..element.depth.min(MAX_INDENT) {
                line.push_str("  ");
            }
            let _ = write!(
                line,
                "{} {:08x} {}",
                element.type_name,
                element.id.0 as u32,
                element.paint.label()
            );
            lines.push(line);
        }
        for line in &mut lines {
            fit(line);
        }
        lines
    }
}

fn outline(bounds: Bounds, background: Rgba, border: Rgba, width: f32) -> Primitive {
    Primitive::Quad {
        bounds,
        background,
        border_color: border,
        border_widths: Edges::all(width),
        corner_radii: Corners::all(0.0),
    }
}

/// Cut `line` to what fits in the panel.
fn fit(line: &mut String) {
    if let Some((end, _)) = line.char_indices().nth(LINE_CHARS) {
        line.truncate(end);
        line.pop();
        line.push('…');
    }
}

/// `rui::elements::div::Div` as `Div`, also inside generic arguments.
fn short_type_name(name: &str) -> String {
    let mut short = String::with_capacity(name.len());
    // Where the path being read starts in `short`.
    let mut path = 0;
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            short.truncate(path);
        } else {
            short.push(c);
            if !(c.is_alphanumeric() || c == '_') {
                path = short.len();
            }
        }
    }
    short
}

fn style_summary(style: &Style) -> String {
    let mut summary = String::new();
    let mut add = |part: std::fmt::Arguments| {
        if !summary.is_empty() {
            summary.push_str("  ");
        }
        let _ = summary.write_fmt(part);
    };
    match style.display {
        Display::None => add(format_args!("hidden")),
        Display::Block => add(format_args!("block")),
        Display::Flex => add(format_args!(
            "{}",
            match style.flex_direction {
                FlexDirection::Row => "row",
                FlexDirection::Column => "col",
                FlexDirection::RowReverse => "row-rev",
                FlexDirection::ColumnReverse => "col-rev",
            }
        )),
    }
    if let Some(width) = style.width {
        add(format_args!("w {width}"));
    }
    if let Some(height) = style.height {
        add(format_args!("h {height}"));
    }
    if style.padding != Edges::ZERO {
        add(format_args!("pad {}", edges(style.padding)));
    }
    if style.margin != Edges::ZERO {
        add(format_args!("margin {}", edges(style.margin)));
    }
    if style.gap != 0.0 {
        add(format_args!("gap {}", style.gap));
    }
    if style.flex_grow != 0.0 {
        add(format_args!("grow {}", style.flex_grow));
    }
    match style.background {
        Background::None => {}
        Background::Solid(color) => {
            let rgba = color.to_rgba();
            let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            add(format_args!(
                "bg #{:02x}{:02x}{:02x}{:02x}",
                channel(rgba.r),
                channel(rgba.g),
                channel(rgba.b),
                channel(rgba.a)
            ));
        }
        _ => add(format_args!("bg gradient")),
    }
    if style.border.width != Edges::ZERO {
        add(format_args!("border {}", edges(style.border.width)));
    }
    if style.opacity != 1.0 {
        add(format_args!("opacity {}", style.opacity));
    }
    if (style.overflow_x, style.overflow_y) != (Overflow::Visible, Overflow::Visible) {
        add(format_args!(
            "overflow {:?}/{:?}",
            style.overflow_x, style.overflow_y
        ));
    }
    summary
}

fn edges(edges: Edges) -> String {
    let Edges {
        top,
        right,
        bottom,
        left,
    } = edges;
    if top == right && right == bottom && bottom == left {
        format!("{top}")
    } else {
        format!("{top} {right} {bottom} {left}")
    }
}

#[cfg(test)]
mod tests {
    use super::short_type_name;

    #[test]
    fn type_names_drop_module_paths() {
        assert_eq!(short_type_name("rui::elements::div::Div"), "Div");
        assert_eq!(
            short_type_name("rui::a::List<rui::b::Row, alloc::string::String>"),
            "List<Row, String>"
        );
        assert_eq!(short_type_name("Plain"), "Plain");
    }
}
