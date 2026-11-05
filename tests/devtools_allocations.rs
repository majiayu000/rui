//! With `CountingAllocator` installed the profiler counts each frame's
//! allocations, leaving its own overlay out.

use rui::devtools::CountingAllocator;
use rui::testing::mount;
use rui::{AppContext, Div, Size, div, text};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn app(_: &mut AppContext) -> Div {
    div().child(text("counted"))
}

#[test]
fn frames_report_their_allocations() {
    let mut session = mount(Size::new(320.0, 200.0), app).expect("mount");
    session.app_context_mut().toggle_profiler();
    session.frame().expect("first profiled frame");
    let first = session.profiler().and_then(|p| p.last_frame()).copied();
    let first = first.and_then(|frame| frame.allocations);
    assert!(first.is_some(), "allocations should be counted");
}
