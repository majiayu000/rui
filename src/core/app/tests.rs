use super::*;
use crate::elements::{Text, text};
use std::cell::Cell;
use std::rc::Rc;

struct CountingView {
    renders: Rc<Cell<u32>>,
    notify_during_render: bool,
}

impl View for CountingView {
    type Element = Text;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        self.renders.set(self.renders.get() + 1);
        if self.notify_during_render {
            cx.notify();
        }
        text(format!("renders: {}", self.renders.get()))
    }
}

fn runtime_view(context: &mut AppContext, view: CountingView) -> RuntimeView<CountingView> {
    let marker = context.create(());
    let entity = Entity::<CountingView>::new(marker.id());
    let notifier = ViewNotifier::new();
    context.set_runtime_view_notifier(entity.id(), notifier.clone());
    RuntimeView::new(entity, notifier, view)
}

#[test]
fn runtime_view_render_invokes_view_once() {
    let renders = Rc::new(Cell::new(0));
    let mut context = AppContext::new();
    let view = CountingView {
        renders: Rc::clone(&renders),
        notify_during_render: false,
    };
    let mut runtime = runtime_view(&mut context, view);

    let _rendered = runtime.render(&mut context);

    assert_eq!(renders.get(), 1);
}

#[test]
fn view_context_notify_marks_current_view_pending() {
    let mut context = AppContext::new();
    let view = CountingView {
        renders: Rc::new(Cell::new(0)),
        notify_during_render: true,
    };
    let mut runtime = runtime_view(&mut context, view);
    let entity_id = runtime.entity.id();

    let _rendered = runtime.render(&mut context);

    assert!(context.frame.pending_updates.contains(&entity_id));
    assert!(context.frame.needs_rebuild);
    assert!(context.frame.dirty);
}

#[test]
fn runtime_view_notifier_schedules_rebuild() {
    let mut context = AppContext::new();
    let notifier = ViewNotifier::new();
    let marker = context.create(());

    context.set_runtime_view_notifier(marker.id(), notifier.clone());
    notifier.notify();

    assert!(context.consume_runtime_view_notification());
    assert!(context.frame.pending_updates.contains(&marker.id()));
    assert!(context.frame.needs_rebuild);
    assert!(context.frame.dirty);
}

#[test]
fn viewport_size_tracks_open_window_and_updates() {
    let mut context = AppContext::new();

    assert_eq!(context.viewport_size(), Size::ZERO);

    context.register_window(WindowOptions::new().size(320.0, 240.0));
    assert_eq!(context.viewport_size(), Size::new(320.0, 240.0));

    context.set_viewport_size(Size::new(640.0, 480.0));
    assert_eq!(context.viewport_size(), Size::new(640.0, 480.0));
}

#[test]
fn runtime_view_renders_again_after_notification() {
    let renders = Rc::new(Cell::new(0));
    let mut context = AppContext::new();
    let view = CountingView {
        renders: Rc::clone(&renders),
        notify_during_render: false,
    };
    let mut runtime = runtime_view(&mut context, view);
    let notifier = runtime.notifier.clone();

    let _first_render = runtime.render(&mut context);
    context.frame.pending_updates.clear();
    context.frame.needs_rebuild = false;
    context.frame.dirty = false;

    notifier.notify();
    assert!(context.consume_runtime_view_notification());
    let _second_render = runtime.render(&mut context);

    assert_eq!(renders.get(), 2);
}

#[test]
fn notify_during_render_survives_processed_pending_clear() {
    let mut context = AppContext::new();
    let view = CountingView {
        renders: Rc::new(Cell::new(0)),
        notify_during_render: true,
    };
    let mut runtime = runtime_view(&mut context, view);
    let entity_id = runtime.entity.id();

    context.notify(entity_id);
    context.frame.pending_updates.clear();
    context.frame.needs_rebuild = false;
    let _rendered = runtime.render(&mut context);
    if !context.frame.pending_updates.is_empty() {
        context.frame.needs_rebuild = true;
    }

    assert!(context.frame.pending_updates.contains(&entity_id));
    assert!(context.frame.needs_rebuild);
}

#[test]
fn redraw_scheduler_counts_sources_and_coalesces_platform_requests() {
    let mut context = AppContext::new();
    context.frame.pending_updates.clear();
    context.frame.needs_rebuild = false;
    context.frame.dirty = false;

    assert!(context.request_platform_redraw_from(RedrawSource::Element));
    assert!(!context.request_platform_redraw_from(RedrawSource::Element));
    assert!(!context.request_platform_redraw_from(RedrawSource::ViewNotification));

    let counts = context.redraw_source_counts();
    assert_eq!(counts.element, 2);
    assert_eq!(counts.view_notification, 1);
    assert!(context.frame.dirty);
    assert!(context.platform_redraw_pending());

    context.complete_redraw_frame();
    assert!(!context.frame.dirty);
    assert!(!context.platform_redraw_pending());
    assert!(context.request_platform_redraw_from(RedrawSource::PlatformLifecycle));
}
