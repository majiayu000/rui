//! One open window: its events, frame and present, behind [`NativeWindowDriver`].

use super::*;

/// What a window asked of the app loop in one iteration.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct WindowRequests {
    pub(super) close: bool,
}

/// One open window, behind the type the app loop stores.
pub(crate) trait NativeWindowDriver {
    fn id(&self) -> WindowId;
    fn mac_window(&self) -> &MacWindow;
    fn mac_window_mut(&mut self) -> &mut MacWindow;
    /// Handle this window's events and draw it if it has work. `context` has
    /// this window active. `held_since` is when the first of `events` arrived,
    /// if they were held for this frame.
    fn run_iteration(
        &mut self,
        context: &mut AppContext,
        events: Vec<MacWindowEvent>,
        held_since: Option<Instant>,
    ) -> WindowRequests;
}

pub(super) struct NativeWindow<F, E> {
    id: WindowId,
    window: MacWindow,
    /// Set at the first present, from `pending_renderer`.
    renderer: Option<MetalRenderer>,
    pending_renderer: Option<PendingMetalRenderer>,
    /// This window's last frame, so each frame redraws only what changed.
    retained: RetainedTarget,
    presenter: Presenter<E>,
    build_root: F,
    last_viewport_size: Size,
    profile_recorder: Option<RendererTelemetryRecorder>,
    automation: Option<NativeDogfoodAutomation>,
    ime_state: crate::platform::mac::ime_state::NativeImeState,
}

/// Attach `build_root` to a created platform window. `context` has the window
/// active, so the first build sees its viewport.
pub(crate) fn native_window<F, E>(
    context: &mut AppContext,
    parts: NativeWindowParts,
    mut build_root: F,
) -> Box<dyn NativeWindowDriver>
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    // AppContext is the single owner of viewport size; the window options only
    // provide its initial value.
    context.set_viewport_size(parts.viewport_size);
    let root = build_root(context);
    let mut presenter = Presenter::with_root(parts.viewport_size, root);
    presenter.set_accessibility_announcements_enabled(true);
    Box::new(NativeWindow {
        id: parts.id,
        window: parts.window,
        renderer: None,
        pending_renderer: Some(parts.pending_renderer),
        retained: RetainedTarget::new(),
        presenter,
        build_root,
        last_viewport_size: parts.viewport_size,
        profile_recorder: RendererTelemetryRecorder::enabled_from_env(),
        automation: parts.automation,
        ime_state: Default::default(),
    })
}

impl<F, E> NativeWindowDriver for NativeWindow<F, E>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    fn id(&self) -> WindowId {
        self.id
    }

    fn mac_window(&self) -> &MacWindow {
        &self.window
    }

    fn mac_window_mut(&mut self) -> &mut MacWindow {
        &mut self.window
    }

    fn run_iteration(
        &mut self,
        context: &mut AppContext,
        mut platform_events: Vec<MacWindowEvent>,
        held_since: Option<Instant>,
    ) -> WindowRequests {
        let NativeWindow {
            window,
            renderer,
            pending_renderer,
            retained,
            presenter,
            build_root,
            last_viewport_size,
            profile_recorder,
            automation: native_dogfood_automation,
            ime_state: native_ime_state,
            ..
        } = self;
        let mut requests = WindowRequests::default();
        let mut phases = RendererFramePhaseDurations::default();
        let mut viewport_size = match window.content_size() {
            Ok(size) => size,
            Err(err) => panic!("failed to read platform window size: {}", err),
        };
        let mut ordered_input_events = Vec::new();
        let mut focus_changed = None;
        let mut close_requested = false;
        let mut automation_focused_element = None;
        if let Some(automation) = native_dogfood_automation {
            automation_focused_element =
                automation.append_events(window, presenter.scene(), &mut platform_events);
        }
        let event_observed_at = if platform_events.is_empty() {
            None
        } else {
            Some(held_since.unwrap_or_else(Instant::now))
        };
        let present_profile = profile_recorder.as_ref().map(|recorder| {
            let event_observed_at = event_observed_at.map(|observed_at| {
                present_latency::media_time() - observed_at.elapsed().as_secs_f64()
            });
            (recorder.next_frame_index(), event_observed_at)
        });

        for event in platform_events {
            match event {
                MacWindowEvent::Accessibility(request) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    ordered_input_events.push(OrderedInputEvent::Accessibility(request));
                }
                MacWindowEvent::Text(command) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    ordered_input_events.push(OrderedInputEvent::Text(command));
                }
                MacWindowEvent::FileDrag(event) => {
                    context.mark_redraw_from(RedrawSource::PlatformInput);
                    ordered_input_events.push(OrderedInputEvent::FileDrag(event));
                }
                // The app loop takes display ticks before windows see events.
                MacWindowEvent::DisplayTick => {}
                MacWindowEvent::Platform(event) => {
                    mark_platform_event_redraw(&event, context);
                    match event {
                        PlatformWindowEvent::Created | PlatformWindowEvent::RedrawRequested => {}
                        PlatformWindowEvent::Resized(size) => viewport_size = size,
                        PlatformWindowEvent::ScaleFactorChanged(_)
                        | PlatformWindowEvent::ApplicationActivated(_)
                        | PlatformWindowEvent::Minimized(_) => {}
                        PlatformWindowEvent::ReopenRequested => {
                            if let Err(err) = window.show() {
                                panic!("failed to reopen platform window: {}", err);
                            }
                        }
                        PlatformWindowEvent::FocusChanged(focused) => {
                            focus_changed = Some(focused);
                        }
                        PlatformWindowEvent::CloseRequested => {
                            close_requested = true;
                            requests.close = true;
                        }
                        PlatformWindowEvent::QuitRequested => {
                            close_requested = true;
                            context.quit();
                        }
                        PlatformWindowEvent::Input(input) => {
                            append_input_event(input, &mut ordered_input_events)
                        }
                    }
                }
            }
        }

        let viewport_changed =
            synchronize_viewport_after_platform_events(context, viewport_size, *last_viewport_size);
        if viewport_changed {
            schedule_platform_redraw(&*window, context, RedrawSource::PlatformResize);
        }

        let frame_events = NativeFrameEvents {
            viewport_changed,
            viewport_size,
            focus_changed,
            close_requested,
            automation_focused_element,
            ordered_input_events,
        };
        let mut resize_applied = false;

        // The stage order lives in FrameStage::ORDER, shared with the
        // headless runner. This runner only supplies the two stages that
        // depend on the platform: which events arrived, and which backend
        // receives the painted scene.
        let outcome = match FramePipeline::run_frame(
            context,
            presenter,
            build_root,
            viewport_size,
            IdlePolicy::SkipWhenIdle,
            |presenter, context| {
                resize_applied = dispatch_native_events(
                    presenter,
                    context,
                    window,
                    &frame_events,
                    native_ime_state,
                );
                Ok(())
            },
            |presenter, _context| {
                let renderer = match (&mut *renderer, pending_renderer.take()) {
                    (Some(renderer), _) => renderer,
                    (slot @ None, Some(pending)) => {
                        let created = match pending.wait() {
                            Ok(renderer) => renderer,
                            Err(err) => panic!("failed to create renderer: {}", err),
                        };
                        window.set_device(created.device());
                        slot.insert(created)
                    }
                    (None, None) => unreachable!("the renderer is created before it is taken"),
                };
                let drawable_wait_started_at = Instant::now();
                let metal_drawable = window.next_drawable();
                phases.drawable_wait_ns = drawable_wait_started_at.elapsed().as_nanos();

                let render_started_at = Instant::now();
                phases.event_to_render_latency_ns = event_observed_at.map(|observed_at| {
                    render_started_at
                        .saturating_duration_since(observed_at)
                        .as_nanos()
                });
                renderer.set_profiling(presenter.profiler().is_some());
                let presentation = match metal_drawable {
                    Some(metal_drawable) => {
                        if let Some((frame_index, event_observed_at)) = present_profile {
                            present_latency::track(&metal_drawable, frame_index, event_observed_at);
                        }
                        // Hold the owned retain across render+present; MetalRenderer
                        // still takes &MetalDrawableRef via Deref.
                        renderer
                            .render_retained(
                                presenter.scene(),
                                retained,
                                &metal_drawable,
                                viewport_size,
                            )
                            .map_err(|err| {
                                FramePipelineError::stage(
                                    crate::core::frame_pipeline::FrameStage::Present,
                                    err.to_string(),
                                )
                            })?;
                        FramePresentation::Presented(Some(renderer.diagnostics()))
                    }
                    None => FramePresentation::Deferred,
                };
                phases.render_ns = render_started_at.elapsed().as_nanos();
                if let Some(profiler) = presenter.profiler_mut() {
                    let stats = renderer.last_frame_stats();
                    profiler.record_render(RenderSample {
                        drawable_wait_ns: phases.drawable_wait_ns,
                        render_ns: phases.render_ns,
                        scheduled_wait_ns: renderer
                            .scheduled_wait()
                            .map_or(0, |time| time.as_nanos()),
                        draw_calls: stats.draw_calls,
                        instances: stats.instances,
                        redrawn_pixels: stats.redrawn_pixels,
                        gpu_ns: renderer.completed_gpu_time().map(|time| time.as_nanos()),
                    });
                }
                Ok(presentation)
            },
        ) {
            Ok(Some(outcome)) => outcome,
            Ok(None) => return requests,
            Err(err) => panic!("frame failed: {err}"),
        };

        if resize_applied {
            *last_viewport_size = viewport_size;
        }
        phases.layout_ns = outcome.durations.layout_ns;
        phases.dispatch_ns = outcome.durations.dispatch_ns;
        phases.paint_ns = outcome.durations.paint_ns;
        let frame_committed = outcome.presented;

        let (ime_sync_result, ime_redraw_requested) =
            crate::platform::mac::ime_state::sync_text_input_snapshot(
                presenter,
                window,
                native_ime_state,
            );
        if let Err(err) = ime_sync_result {
            log::error!("failed to synchronize native text input state: {err}");
        }
        // Invalid-snapshot cancel can mutate the input after Paint/Present; schedule a
        // platform redraw so cleared marked text is painted before the run loop sleeps.
        if ime_redraw_requested {
            schedule_platform_redraw(&*window, context, RedrawSource::PlatformInput);
        }

        let accessibility_bridge = window.accessibility_bridge_mut();
        let tree_published = match presenter.accessibility_tree() {
            Ok(accessibility_tree) => {
                match accessibility_bridge.publish_tree(&accessibility_tree) {
                    Ok(()) => Some(accessibility_tree),
                    Err(err) => {
                        log::error!("failed to publish accessibility tree: {err}");
                        None
                    }
                }
            }
            Err(err) => {
                log::error!("failed to build accessibility tree: {err}");
                None
            }
        };
        // FocusChanged is tree-dependent and stays queued until a matching tree
        // publishes. ActionFeedback posts on the content view and is announced
        // immediately so retained-tree controls still confirm activation during
        // recovery instead of buffering a burst for the next successful publish.
        if let Some(published_tree) = tree_published.as_ref() {
            let focused = presenter.focused_element();
            for announcement in filter_retained_accessibility_announcements(
                presenter.take_accessibility_announcements(),
                published_tree,
                focused,
            ) {
                if let Err(err) = accessibility_bridge.announce(&announcement) {
                    log::error!("failed to publish accessibility announcement: {err}");
                }
            }
        } else {
            let pending = presenter.take_accessibility_announcements();
            let mut retained_focus = Vec::new();
            for announcement in pending {
                match announcement.kind() {
                    AccessibilityAnnouncementKind::ActionFeedback => {
                        if let Err(err) = accessibility_bridge.announce(&announcement) {
                            log::error!("failed to publish accessibility announcement: {err}");
                        }
                    }
                    AccessibilityAnnouncementKind::FocusChanged => {
                        retained_focus.push(announcement);
                    }
                }
            }
            presenter.retain_accessibility_announcements(retained_focus);
        }

        if frame_committed && let Some(recorder) = profile_recorder.as_mut() {
            let diagnostics = match presenter.renderer_diagnostics() {
                Some(diagnostics) => diagnostics,
                None => panic!("renderer diagnostics were not recorded for this frame"),
            };
            let telemetry = recorder.capture_telemetry(
                "metal",
                viewport_size,
                phases,
                diagnostics,
                RendererBatchDiagnostics::for_metal_scene(presenter.scene()),
            );
            eprintln!("{}", telemetry.to_json_line());
        }

        if mark_deferred_frame_for_retry(context, frame_committed) {
            request_automation_redraw(&*window);
        }
        requests
    }
}
