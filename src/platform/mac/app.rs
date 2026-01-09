//! macOS application runner

use crate::core::app::AppContext;
use crate::core::geometry::Bounds;
use crate::core::window::WindowOptions;
use crate::elements::element::{Element, LayoutContext, PaintContext};
use crate::platform::mac::window::create_window;
use crate::renderer::metal::MetalRenderer;
use crate::renderer::Scene;
use objc2::msg_send;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::NSRunLoop;
use taffy::prelude::*;

/// Run the application with default window options
pub fn run_app<E: Element + 'static>(context: AppContext, root: E) {
    run_app_with_options(context, root, WindowOptions::default());
}

/// Run the application with custom window options
pub fn run_app_with_options<E: Element + 'static>(
    mut context: AppContext,
    mut root: E,
    options: WindowOptions,
) {
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
        app.activateIgnoringOtherApps(true);

        // Create layout engine
        let mut taffy: TaffyTree<crate::core::ElementId> = TaffyTree::new();

        // Create scene
        let mut scene = Scene::new();

        // Main run loop
        let run_loop = NSRunLoop::currentRunLoop();
        let viewport_size = options.size;

        // Render loop
        loop {
            // Process events
            let _: () = msg_send![&*run_loop, runUntilDate: std::ptr::null::<objc2::runtime::AnyObject>()];

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

            // Paint phase
            scene.clear();
            let mut paint_cx = PaintContext::new(&mut scene, root_bounds);
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
