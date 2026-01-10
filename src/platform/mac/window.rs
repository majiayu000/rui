//! macOS window creation

use crate::core::window::WindowOptions;
use metal::Device;
use objc2::rc::Retained;
use objc2::msg_send;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSRect, NSPoint, NSSize, NSString};
use objc2::MainThreadOnly;
use objc2_quartz_core::CAMetalLayer;
use metal::foreign_types::ForeignType;

/// Create a macOS window with a Metal layer
pub unsafe fn create_window(
    options: &WindowOptions,
    device: &Device,
    mtm: MainThreadMarker,
) -> (Retained<NSWindow>, Retained<CAMetalLayer>) {
    // Define window frame
    let frame = NSRect::new(
        NSPoint::new(100.0, 100.0),
        NSSize::new(options.size.width as f64, options.size.height as f64),
    );

    // Window style
    let mut style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
    if options.resizable {
        style |= NSWindowStyleMask::Resizable;
    }

    // Create window
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            objc2_app_kit::NSBackingStoreType(2), // NSBackingStoreBuffered = 2
            false,
        )
    };

    // Set title
    let title = NSString::from_str(&options.title);
    window.setTitle(&title);

    // Get content view
    let content_view = window.contentView().expect("No content view");

    // Create Metal layer
    let metal_layer = CAMetalLayer::new();

    // Set the Metal device on the layer
    let device_ptr = device.as_ptr() as *mut objc2::runtime::AnyObject;
    let _: () = msg_send![&*metal_layer, setDevice: device_ptr];

    // Configure layer
    let _: () = msg_send![&*metal_layer, setPixelFormat: 80u64]; // MTLPixelFormatBGRA8Unorm = 80
    let _: () = msg_send![&*metal_layer, setFramebufferOnly: true];

    // Set layer size
    let drawable_size = NSSize::new(
        options.size.width as f64 * 2.0, // Retina scale
        options.size.height as f64 * 2.0,
    );
    let _: () = msg_send![&*metal_layer, setDrawableSize: drawable_size];

    // Set content scale factor
    let _: () = msg_send![&*metal_layer, setContentsScale: 2.0f64];

    // Set layer on view
    let _: () = msg_send![&*content_view, setWantsLayer: true];
    let _: () = msg_send![&*content_view, setLayer: &*metal_layer];

    // Center window on screen
    window.center();

    (window, metal_layer)
}
