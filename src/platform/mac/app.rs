//! macOS application runner

use crate::core::app::AppContext;
use crate::core::geometry::Bounds;
use crate::core::window::WindowOptions;
use crate::core::event::{Event, KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Point, Size};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use crate::platform::mac::window::create_window;
use crate::renderer::metal::MetalRenderer;
use crate::renderer::Scene;
use objc2::msg_send;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventMask, NSEventModifierFlags,
    NSEventType,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPoint, NSRect};
use taffy::prelude::*;

/// Run the application with default window options
pub fn run_app<F, E>(context: AppContext, build_root: F)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    run_app_with_options(context, build_root, WindowOptions::default());
}

/// Run the application with custom window options
pub fn run_app_with_options<F, E>(context: AppContext, mut build_root: F, options: WindowOptions)
where
    F: FnMut(&mut AppContext) -> E + 'static,
    E: Element + 'static,
{
    let mut context = context;
    let mut root = build_root(&mut context);

    // Get main thread marker
    let mtm = MainThreadMarker::new().expect("Must be called from main thread");

    unsafe {
        // Initialize NSApplication
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        // Create the renderer
        let renderer = MetalRenderer::new().expect("Failed to create Metal renderer");

        // Create the window with Metal layer
        let (window, metal_layer) = create_window(&options, renderer.device(), mtm);

        // Make key and order front
        let _: () = msg_send![&*window, makeKeyAndOrderFront: std::ptr::null::<objc2::runtime::AnyObject>()];

        // Activate the application
        app.activate();

        // Create layout engine
        let mut taffy: TaffyTree<crate::core::ElementId> = TaffyTree::new();

        // Create scene
        let mut scene = Scene::new();

        // Main run loop
        let mut viewport_size = options.size;
        let mut last_viewport_size = viewport_size;
        let mut last_focused = false;
        let mut focused_element: Option<crate::core::ElementId> = None;
        let mut window_visible = true;

        // Render loop
        let mut needs_rebuild = true;
        loop {
            let expiration = NSDate::distantPast();
            let mask = NSEventMask::MouseMoved
                | NSEventMask::LeftMouseDown
                | NSEventMask::LeftMouseUp
                | NSEventMask::LeftMouseDragged
                | NSEventMask::RightMouseDown
                | NSEventMask::RightMouseUp
                | NSEventMask::RightMouseDragged
                | NSEventMask::ScrollWheel
                | NSEventMask::KeyDown
                | NSEventMask::KeyUp;

            let content_view = window.contentView().expect("No content view");
            let view_bounds: NSRect = msg_send![&*content_view, bounds];
            viewport_size = Size::new(
                view_bounds.size.width as f32,
                view_bounds.size.height as f32,
            );

            let window_number = window.windowNumber();
            let mut pointer_events = Vec::new();
            let mut scroll_events = Vec::new();
            let mut key_events = Vec::new();

            while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                mask,
                Some(&expiration),
                &NSDefaultRunLoopMode,
                true,
            ) {
                let event_type = event.r#type();
                if event.windowNumber() != window_number {
                    app.sendEvent(&event);
                    continue;
                }

                let location: NSPoint = event.locationInWindow();
                let position = Point::new(
                    location.x as f32,
                    (view_bounds.size.height - location.y) as f32,
                );

                if event_type == NSEventType::LeftMouseDown {
                    pointer_events.push(PointerEvent {
                        kind: PointerEventKind::Down,
                        position,
                        button: Some(MouseButton::Left),
                    });
                } else if event_type == NSEventType::LeftMouseUp {
                    pointer_events.push(PointerEvent {
                        kind: PointerEventKind::Up,
                        position,
                        button: Some(MouseButton::Left),
                    });
                } else if event_type == NSEventType::RightMouseDown {
                    pointer_events.push(PointerEvent {
                        kind: PointerEventKind::Down,
                        position,
                        button: Some(MouseButton::Right),
                    });
                } else if event_type == NSEventType::RightMouseUp {
                    pointer_events.push(PointerEvent {
                        kind: PointerEventKind::Up,
                        position,
                        button: Some(MouseButton::Right),
                    });
                } else if event_type == NSEventType::MouseMoved
                    || event_type == NSEventType::LeftMouseDragged
                    || event_type == NSEventType::RightMouseDragged
                {
                    let button = if event_type == NSEventType::LeftMouseDragged {
                        Some(MouseButton::Left)
                    } else if event_type == NSEventType::RightMouseDragged {
                        Some(MouseButton::Right)
                    } else {
                        None
                    };
                    pointer_events.push(PointerEvent {
                        kind: PointerEventKind::Move,
                        position,
                        button,
                    });
                } else if event_type == NSEventType::ScrollWheel {
                    let delta_x = event.scrollingDeltaX() as f32;
                    let delta_y = event.scrollingDeltaY() as f32;
                    scroll_events.push(ScrollEvent {
                        position,
                        delta_x,
                        delta_y,
                        modifiers: modifiers_from_event(&event),
                    });
                } else if event_type == NSEventType::KeyDown || event_type == NSEventType::KeyUp {
                    let key_event = key_event_from_event(&event);
                    key_events.push((event_type == NSEventType::KeyDown, key_event));
                }

                app.sendEvent(&event);
            }

            if needs_rebuild || !context.pending_updates.is_empty() {
                root = build_root(&mut context);
                context.pending_updates.clear();
                needs_rebuild = false;
            }

            // Rebuild layout tree each frame to avoid unbounded growth
            taffy.clear();

            // Layout phase
            let mut layout_cx = LayoutContext::new(&mut taffy, viewport_size);
            let root_node = root.layout(&mut layout_cx);

            // Compute layout
            taffy
                .compute_layout(
                    root_node,
                    taffy::Size {
                        width: AvailableSpace::Definite(viewport_size.width),
                        height: AvailableSpace::Definite(viewport_size.height),
                    },
                )
                .expect("Layout failed");

            // Get computed bounds
            let layout = taffy.layout(root_node).expect("No layout");
            let root_bounds = Bounds::from_xywh(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
            );

            if viewport_size != last_viewport_size {
                root.handle_window_event(&Event::WindowResize {
                    width: viewport_size.width,
                    height: viewport_size.height,
                });
                last_viewport_size = viewport_size;
            }

            let is_focused: bool = msg_send![&*window, isKeyWindow];
            if is_focused != last_focused {
                let evt = if is_focused {
                    Event::Focus(crate::core::event::FocusEvent { focused: true })
                } else {
                    focused_element = None;
                    Event::Blur(crate::core::event::FocusEvent { focused: false })
                };
                root.handle_window_event(&evt);
                last_focused = is_focused;
            }

            let is_visible: bool = msg_send![&*window, isVisible];
            if window_visible && !is_visible {
                root.handle_window_event(&Event::WindowClose);
                context.quit();
                window_visible = false;
            }

            let mut event_cx = EventContext::new(root_bounds, &taffy, &mut focused_element);

            for event in &pointer_events {
                root.handle_pointer_event(&mut event_cx, event);
            }

            for event in &scroll_events {
                root.handle_scroll_event(&mut event_cx, event);
            }

            for (is_down, event) in &key_events {
                let _ = is_down;
                root.handle_key_event(&mut event_cx, event);
            }

            // Paint phase
            scene.clear();
            let mut paint_cx = PaintContext::new(&mut scene, root_bounds, &taffy);
            root.paint(&mut paint_cx);
            scene.finish();

            // Get next drawable from Metal layer
            let layer_ptr = objc2::rc::Retained::as_ptr(&metal_layer) as *mut objc2::runtime::AnyObject;
            let drawable: *mut objc2::runtime::AnyObject = msg_send![layer_ptr, nextDrawable];

            if !drawable.is_null() {
                // Render the scene - cast to metal crate's type
                let metal_drawable = std::mem::transmute::<*mut objc2::runtime::AnyObject, &metal::MetalDrawableRef>(drawable);
                renderer.render(&scene, metal_drawable, viewport_size);
            }

            // Check if we should quit
            if !context.is_running() {
                break;
            }

            // Small sleep to prevent spinning
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}

fn modifiers_from_event(event: &NSEvent) -> Modifiers {
    let flags = event.modifierFlags();
    Modifiers {
        shift: flags.contains(NSEventModifierFlags::Shift),
        ctrl: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        meta: flags.contains(NSEventModifierFlags::Command),
    }
}

fn key_event_from_event(event: &NSEvent) -> KeyEvent {
    let modifiers = modifiers_from_event(event);
    let mut key_event = KeyEvent::new(KeyCode::Unknown(event.keyCode() as u32), modifiers);
    key_event.is_repeat = event.isARepeat();

    if let Some(chars) = event.charactersIgnoringModifiers() {
        if let Some(ch) = chars.to_string().chars().next() {
            key_event.key = keycode_from_char(ch);
        }
    }

    if let Some(chars) = event.characters() {
        if let Some(ch) = chars.to_string().chars().next() {
            if !ch.is_control() {
                key_event.char = Some(ch);
            }
        }
    }

    key_event
}

fn keycode_from_char(ch: char) -> KeyCode {
    match ch {
        '\r' | '\n' => KeyCode::Enter,
        '\u{7f}' => KeyCode::Backspace,
        '\u{1b}' => KeyCode::Escape,
        '\t' => KeyCode::Tab,
        ' ' => KeyCode::Space,
        '0' => KeyCode::Key0,
        '1' => KeyCode::Key1,
        '2' => KeyCode::Key2,
        '3' => KeyCode::Key3,
        '4' => KeyCode::Key4,
        '5' => KeyCode::Key5,
        '6' => KeyCode::Key6,
        '7' => KeyCode::Key7,
        '8' => KeyCode::Key8,
        '9' => KeyCode::Key9,
        'a' | 'A' => KeyCode::A,
        'b' | 'B' => KeyCode::B,
        'c' | 'C' => KeyCode::C,
        'd' | 'D' => KeyCode::D,
        'e' | 'E' => KeyCode::E,
        'f' | 'F' => KeyCode::F,
        'g' | 'G' => KeyCode::G,
        'h' | 'H' => KeyCode::H,
        'i' | 'I' => KeyCode::I,
        'j' | 'J' => KeyCode::J,
        'k' | 'K' => KeyCode::K,
        'l' | 'L' => KeyCode::L,
        'm' | 'M' => KeyCode::M,
        'n' | 'N' => KeyCode::N,
        'o' | 'O' => KeyCode::O,
        'p' | 'P' => KeyCode::P,
        'q' | 'Q' => KeyCode::Q,
        'r' | 'R' => KeyCode::R,
        's' | 'S' => KeyCode::S,
        't' | 'T' => KeyCode::T,
        'u' | 'U' => KeyCode::U,
        'v' | 'V' => KeyCode::V,
        'w' | 'W' => KeyCode::W,
        'x' | 'X' => KeyCode::X,
        'y' | 'Y' => KeyCode::Y,
        'z' | 'Z' => KeyCode::Z,
        _ => {
            let code = ch as u32;
            match code {
                0xF700 => KeyCode::ArrowUp,
                0xF701 => KeyCode::ArrowDown,
                0xF702 => KeyCode::ArrowLeft,
                0xF703 => KeyCode::ArrowRight,
                0xF729 => KeyCode::Home,
                0xF72B => KeyCode::End,
                0xF72C => KeyCode::PageUp,
                0xF72D => KeyCode::PageDown,
                0xF728 => KeyCode::Delete,
                _ => KeyCode::Unknown(code),
            }
        }
    }
}
