//! The pointer cases of `perf_gate`: moves over the long list, whose rows have
//! no handlers.

use super::{Cost, VIEWPORT, list, measure};
use rui::elements::element::{PointerEvent, PointerEventKind};
use rui::{Point, Presenter, Size};
use std::collections::BTreeMap;

/// A move down the list, 13 px further each round.
fn move_to(round: usize) -> PointerEvent {
    PointerEvent {
        kind: PointerEventKind::Move,
        position: Point::new(200.0, 10.0 + round as f32 * 13.0),
        button: None,
    }
}

/// `list.pointer_move` and `list.pointer_frame`.
pub(super) fn measure_pointer_moves(costs: &mut BTreeMap<&'static str, Cost>) {
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let mut presenter = Presenter::with_root(viewport, list(2_000, 0));
    for _ in 0..2 {
        presenter.layout(viewport).expect("layout");
        presenter.paint();
    }
    // Dispatched after paint, outside a frame: the painted scene is current, so
    // the move only queries it. No runner takes this path. Each frame lays out
    // before it dispatches (`FrameStage::ORDER`), which leaves the scene stale.
    costs.insert(
        "list.pointer_move",
        measure(|round| {
            presenter.dispatch_pointer_event(&move_to(round));
        }),
    );
    // The path the runners take: layout, then the move, which first paints the
    // whole tree into a scene of its own to find what it hits (#246).
    costs.insert(
        "list.pointer_frame",
        measure(|round| {
            presenter.layout(viewport).expect("layout");
            presenter.dispatch_pointer_event(&move_to(round));
        }),
    );
}
