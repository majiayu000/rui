use crate::core::ElementId;
use crate::core::geometry::{Bounds, Point, Size};
use crate::elements::element::PointerEventKind;
use crate::renderer::Scene;
use crate::renderer::text::TextMeasureCache;
use taffy::prelude::TaffyTree;

#[derive(Debug, Clone, PartialEq)]
pub struct PresenterFrame {
    pub viewport_size: Size,
    pub root_bounds: Bounds,
    pub primitive_count: usize,
}

pub struct Presenter {
    viewport_size: Size,
    taffy: TaffyTree<ElementId>,
    scene: Scene,
    text_measurer: TextMeasureCache,
    root_bounds: Bounds,
    focused_element: Option<ElementId>,
    last_pointer_hit_target: Option<ElementId>,
    pointer_capture_target: Option<ElementId>,
    last_frame: Option<PresenterFrame>,
}

impl Presenter {
    pub fn new(viewport_size: Size) -> Self {
        Self {
            viewport_size,
            taffy: TaffyTree::new(),
            scene: Scene::new(),
            text_measurer: TextMeasureCache::new(),
            root_bounds: Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height),
            focused_element: None,
            last_pointer_hit_target: None,
            pointer_capture_target: None,
            last_frame: None,
        }
    }

    pub fn viewport_size(&self) -> Size {
        self.viewport_size
    }

    pub fn set_viewport_size(&mut self, viewport_size: Size) {
        self.viewport_size = viewport_size;
    }

    pub fn root_bounds(&self) -> Bounds {
        self.root_bounds
    }

    pub fn set_root_bounds(&mut self, root_bounds: Bounds) {
        self.root_bounds = root_bounds;
    }

    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused_element
    }

    pub fn set_focused_element(&mut self, focused_element: Option<ElementId>) {
        self.focused_element = focused_element;
    }

    pub fn focused_element_mut(&mut self) -> &mut Option<ElementId> {
        &mut self.focused_element
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn taffy(&self) -> &TaffyTree<ElementId> {
        &self.taffy
    }

    pub fn frame_surfaces_mut(&mut self) -> (&mut TaffyTree<ElementId>, &mut Scene) {
        (&mut self.taffy, &mut self.scene)
    }

    pub(crate) fn frame_resources_mut(
        &mut self,
    ) -> (&mut TaffyTree<ElementId>, &mut Scene, &mut TextMeasureCache) {
        (&mut self.taffy, &mut self.scene, &mut self.text_measurer)
    }

    /// Layout inputs owned by the presenter. The text measurement cache is
    /// handed out by reference so its metrics survive across frames.
    pub fn layout_surfaces_mut(&mut self) -> (&mut TaffyTree<ElementId>, &mut TextMeasureCache) {
        (&mut self.taffy, &mut self.text_measurer)
    }

    pub fn paint_surfaces_mut(&mut self) -> (&TaffyTree<ElementId>, &mut Scene) {
        (&self.taffy, &mut self.scene)
    }

    pub fn event_context_parts_mut(
        &mut self,
    ) -> (Bounds, &TaffyTree<ElementId>, &mut Option<ElementId>) {
        (self.root_bounds, &self.taffy, &mut self.focused_element)
    }

    pub fn hit_test(&self, position: Point) -> Option<ElementId> {
        self.scene.hit_test(position)
    }

    pub fn pointer_dispatch_target(&self, hit_target: Option<ElementId>) -> Option<ElementId> {
        self.pointer_capture_target.or(hit_target)
    }

    pub fn previous_pointer_target(&self, kind: PointerEventKind) -> Option<ElementId> {
        if matches!(kind, PointerEventKind::Move) {
            self.last_pointer_hit_target
        } else {
            None
        }
    }

    pub fn update_pointer_tracking(
        &mut self,
        kind: PointerEventKind,
        stopped: bool,
        dispatch_target: Option<ElementId>,
        hit_target: Option<ElementId>,
    ) {
        match kind {
            PointerEventKind::Down if stopped => {
                self.pointer_capture_target = dispatch_target;
            }
            PointerEventKind::Up => {
                self.pointer_capture_target = None;
            }
            PointerEventKind::Move => {
                self.last_pointer_hit_target = hit_target;
            }
            PointerEventKind::Down => {}
        }
    }

    pub fn complete_frame(&mut self) {
        self.last_frame = Some(PresenterFrame {
            viewport_size: self.viewport_size,
            root_bounds: self.root_bounds,
            primitive_count: self.scene.len(),
        });
    }

    pub fn last_frame(&self) -> Option<&PresenterFrame> {
        self.last_frame.as_ref()
    }
}
