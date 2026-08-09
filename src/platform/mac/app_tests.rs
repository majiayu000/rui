use super::*;

/// Redraw source each `PlatformWindowEvent` variant is classified as, or
/// `None` when the event must not mark a redraw at all.
fn classified_source(event: &PlatformWindowEvent) -> Option<RedrawSource> {
    let mut context = AppContext::new();
    let before = context.redraw_source_counts();
    mark_platform_event_redraw(event, &mut context);
    let after = context.redraw_source_counts();

    let changed = [
        (RedrawSource::Explicit, before.explicit, after.explicit),
        (
            RedrawSource::ViewNotification,
            before.view_notification,
            after.view_notification,
        ),
        (RedrawSource::Element, before.element, after.element),
        (
            RedrawSource::PlatformLifecycle,
            before.platform_lifecycle,
            after.platform_lifecycle,
        ),
        (
            RedrawSource::PlatformResize,
            before.platform_resize,
            after.platform_resize,
        ),
        (
            RedrawSource::PlatformScaleFactor,
            before.platform_scale_factor,
            after.platform_scale_factor,
        ),
        (
            RedrawSource::PlatformFocus,
            before.platform_focus,
            after.platform_focus,
        ),
        (
            RedrawSource::PlatformInput,
            before.platform_input,
            after.platform_input,
        ),
        (
            RedrawSource::PlatformRedraw,
            before.platform_redraw,
            after.platform_redraw,
        ),
    ]
    .into_iter()
    .filter(|(_, before, after)| after > before)
    .map(|(source, _, _)| source)
    .collect::<Vec<_>>();

    match changed.as_slice() {
        [] => None,
        [source] => Some(*source),
        many => panic!("one event marked {} redraw sources: {many:?}", many.len()),
    }
}

#[test]
fn every_platform_event_maps_to_its_redraw_source() {
    let cases = [
        (
            PlatformWindowEvent::Created,
            Some(RedrawSource::PlatformLifecycle),
        ),
        (
            PlatformWindowEvent::CloseRequested,
            Some(RedrawSource::PlatformLifecycle),
        ),
        (
            PlatformWindowEvent::QuitRequested,
            Some(RedrawSource::PlatformLifecycle),
        ),
        (
            PlatformWindowEvent::ReopenRequested,
            Some(RedrawSource::PlatformLifecycle),
        ),
        (
            PlatformWindowEvent::Minimized(false),
            Some(RedrawSource::PlatformLifecycle),
        ),
        (PlatformWindowEvent::Minimized(true), None),
        (
            PlatformWindowEvent::Resized(Size::new(320.0, 240.0)),
            Some(RedrawSource::PlatformResize),
        ),
        (
            PlatformWindowEvent::ScaleFactorChanged(2.0),
            Some(RedrawSource::PlatformScaleFactor),
        ),
        (
            PlatformWindowEvent::FocusChanged(true),
            Some(RedrawSource::PlatformFocus),
        ),
        (
            PlatformWindowEvent::FocusChanged(false),
            Some(RedrawSource::PlatformFocus),
        ),
        (
            PlatformWindowEvent::ApplicationActivated(true),
            Some(RedrawSource::PlatformFocus),
        ),
        (
            PlatformWindowEvent::ApplicationActivated(false),
            Some(RedrawSource::PlatformFocus),
        ),
        (
            PlatformWindowEvent::RedrawRequested,
            Some(RedrawSource::PlatformRedraw),
        ),
        (
            PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(KeyEvent::new(
                KeyCode::Enter,
                Modifiers::none(),
            ))),
            Some(RedrawSource::PlatformInput),
        ),
    ];

    for (event, expected) in cases {
        assert_eq!(
            classified_source(&event),
            expected,
            "unexpected redraw source for {event:?}"
        );
    }
}

#[test]
fn minimizing_leaves_the_context_clean_while_restoring_dirties_it() {
    let mut context = AppContext::new();
    context.dirty = false;
    mark_platform_event_redraw(&PlatformWindowEvent::Minimized(true), &mut context);
    assert!(!context.dirty, "minimizing must not request a frame");

    mark_platform_event_redraw(&PlatformWindowEvent::Minimized(false), &mut context);
    assert!(context.dirty, "restoring must request a frame");
}

#[test]
fn native_viewport_resize_sync_requests_one_rebuild() {
    let initial_size = Size::new(320.0, 240.0);
    let resized = Size::new(640.0, 480.0);
    let mut context = AppContext::new();
    context.set_viewport_size(initial_size);
    context.needs_rebuild = false;

    assert!(synchronize_viewport_after_platform_events(
        &mut context,
        resized,
        initial_size,
    ));
    assert_eq!(context.viewport_size(), resized);
    assert!(context.needs_rebuild);

    context.needs_rebuild = false;
    assert!(!synchronize_viewport_after_platform_events(
        &mut context,
        resized,
        resized,
    ));
    assert!(!context.needs_rebuild);
}

#[test]
fn presented_frame_reports_the_viewport_owned_by_app_context() {
    let initial_size = Size::new(320.0, 240.0);
    let resized = Size::new(640.0, 480.0);
    let mut context = AppContext::new();
    context.set_viewport_size(initial_size);
    let mut presenter = Presenter::with_root(initial_size, crate::elements::div());

    synchronize_viewport_after_platform_events(&mut context, resized, initial_size);
    let viewport_size = context.viewport_size();
    if let Err(err) = presenter.layout(viewport_size) {
        panic!("layout failed: {err}");
    }
    presenter.paint();
    presenter.complete_presented_frame(None);

    match presenter.last_frame() {
        Some(frame) => assert_eq!(frame.viewport_size, resized),
        None => panic!("expected a recorded frame"),
    }
}

#[test]
fn deferred_frame_requeues_the_platform_redraw() {
    let mut context = AppContext::new();

    assert!(mark_deferred_frame_for_retry(&mut context, false));
    assert!(context.platform_redraw_pending());
    assert!(!mark_deferred_frame_for_retry(&mut context, true));
}

#[test]
fn ime_commit_events_are_forwarded_as_text_input_events() {
    let mut ordered_input_events = Vec::new();

    append_input_event(
        PlatformInputEvent::Ime(PlatformImeEvent::Commit("你好".to_string())),
        &mut ordered_input_events,
    );

    match ordered_input_events.as_slice() {
        [OrderedInputEvent::Text(TextInputEvent::CommitComposition(text))] => {
            assert_eq!(text, "你好");
        }
        other => panic!("expected one committed text input event, got {other:?}"),
    }
}

#[test]
fn key_and_text_input_events_preserve_platform_order() {
    let mut ordered_input_events = Vec::new();

    append_input_event(
        PlatformInputEvent::Ime(PlatformImeEvent::Commit("done".to_string())),
        &mut ordered_input_events,
    );
    append_input_event(
        PlatformInputEvent::Mouse(PlatformMouseEvent {
            kind: PlatformMouseEventKind::Down,
            position: Point::new(12.0, 18.0),
            button: Some(MouseButton::Left),
        }),
        &mut ordered_input_events,
    );
    append_input_event(
        PlatformInputEvent::Scroll(ScrollEvent {
            position: Point::new(12.0, 18.0),
            delta_x: 0.0,
            delta_y: 4.0,
            modifiers: Modifiers::none(),
        }),
        &mut ordered_input_events,
    );
    append_input_event(
        PlatformInputEvent::KeyDown(KeyEvent::new(KeyCode::Enter, Modifiers::none())),
        &mut ordered_input_events,
    );

    assert!(matches!(
        ordered_input_events.as_slice(),
        [
            OrderedInputEvent::Text(TextInputEvent::CommitComposition(text)),
            OrderedInputEvent::Pointer(pointer),
            OrderedInputEvent::Scroll(scroll),
            OrderedInputEvent::Key {
                is_down: true,
                event
            }
        ] if text == "done"
            && pointer.position == Point::new(12.0, 18.0)
            && scroll.delta_y == 4.0
            && event.key == KeyCode::Enter
    ));
}
