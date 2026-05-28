//! macOS window creation

use crate::core::geometry::Size;
use crate::core::window::WindowOptions;
use crate::platform::window::{
    PlatformRendererAttachment, PlatformRendererTarget, PlatformWindow, PlatformWindowError,
    PlatformWindowFeature, PlatformWindowFeatures, PlatformWindowState, validate_window_options,
};
use metal::Device;
use metal::foreign_types::ForeignType;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CAMetalLayer;

pub(crate) const MAC_REDRAW_EVENT_DATA: isize = 0x5255_4952;
const MAC_REDRAW_EVENT_SUBTYPE: i16 = 0;

pub struct MacWindow {
    window: Retained<NSWindow>,
    metal_layer: Retained<CAMetalLayer>,
}

impl MacWindow {
    pub(crate) fn make_key_and_order_front(&self) {
        unsafe {
            let _: () = msg_send![
                &*self.window,
                makeKeyAndOrderFront: std::ptr::null::<objc2::runtime::AnyObject>()
            ];
        }
    }

    pub(crate) fn window_number(&self) -> isize {
        self.window.windowNumber()
    }

    pub(crate) fn content_size(&self) -> Result<Size, PlatformWindowError> {
        let content_view = self
            .window
            .contentView()
            .ok_or_else(|| PlatformWindowError::backend("macos", "window has no content view"))?;
        let view_bounds: NSRect = unsafe { msg_send![&*content_view, bounds] };
        Ok(Size::new(
            view_bounds.size.width as f32,
            view_bounds.size.height as f32,
        ))
    }

    pub(crate) fn is_focused(&self) -> bool {
        unsafe { msg_send![&*self.window, isKeyWindow] }
    }

    pub(crate) fn is_visible(&self) -> bool {
        unsafe { msg_send![&*self.window, isVisible] }
    }

    pub(crate) fn next_drawable(&self) -> Option<&metal::MetalDrawableRef> {
        let layer_ptr =
            objc2::rc::Retained::as_ptr(&self.metal_layer) as *mut objc2::runtime::AnyObject;
        let drawable: *mut objc2::runtime::AnyObject =
            unsafe { msg_send![layer_ptr, nextDrawable] };

        if drawable.is_null() {
            return None;
        }

        Some(unsafe { &*drawable.cast::<metal::MetalDrawableRef>() })
    }

    fn scale_factor(&self) -> f32 {
        let scale_factor: f64 = unsafe { msg_send![&*self.window, backingScaleFactor] };
        scale_factor as f32
    }
}

impl PlatformWindow for MacWindow {
    fn platform_name(&self) -> &'static str {
        "macos"
    }

    fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures {
            lifecycle: true,
            input_events: true,
            dpi: true,
            resizing: true,
            focus: true,
            clipboard: false,
            renderer_attachment: true,
        }
    }

    fn state(&self) -> Result<PlatformWindowState, PlatformWindowError> {
        Ok(PlatformWindowState {
            size: self.content_size()?,
            scale_factor: self.scale_factor(),
            focused: self.is_focused(),
            visible: self.is_visible(),
            renderer_attached: true,
        })
    }

    fn show(&mut self) -> Result<(), PlatformWindowError> {
        self.make_key_and_order_front();
        Ok(())
    }

    fn set_title(&mut self, title: &str) -> Result<(), PlatformWindowError> {
        let title = NSString::from_str(title);
        self.window.setTitle(&title);
        Ok(())
    }

    fn set_size(&mut self, size: Size) -> Result<(), PlatformWindowError> {
        if size.width <= 0.0 || size.height <= 0.0 {
            return Err(PlatformWindowError::invalid_options(
                "window width and height must be greater than zero",
            ));
        }

        let content_size = NSSize::new(size.width as f64, size.height as f64);
        unsafe {
            let _: () = msg_send![&*self.window, setContentSize: content_size];
        }
        Ok(())
    }

    fn set_focus(&mut self, focused: bool) -> Result<(), PlatformWindowError> {
        unsafe {
            if focused {
                self.make_key_and_order_front();
            } else {
                let _: () = msg_send![&*self.window, resignKeyWindow];
            }
        }
        Ok(())
    }

    fn renderer_attachment(&self) -> Result<PlatformRendererAttachment, PlatformWindowError> {
        Ok(PlatformRendererAttachment {
            target: PlatformRendererTarget::MetalLayer,
            viewport_size: self.content_size()?,
            scale_factor: self.scale_factor(),
        })
    }

    fn request_redraw(&self) -> Result<(), PlatformWindowError> {
        let content_view = self
            .window
            .contentView()
            .ok_or_else(|| PlatformWindowError::backend("macos", "window has no content view"))?;
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            PlatformWindowError::backend("macos", "redraw requested off the main thread")
        })?;
        let app = NSApplication::sharedApplication(mtm);
        unsafe {
            let _: () = msg_send![&*content_view, setNeedsDisplay: true];
        }
        let event =
            NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::ApplicationDefined,
                NSPoint::new(0.0, 0.0),
                NSEventModifierFlags::empty(),
                NSDate::timeIntervalSinceReferenceDate_class(),
                self.window_number(),
                None,
                MAC_REDRAW_EVENT_SUBTYPE,
                MAC_REDRAW_EVENT_DATA,
                0,
            )
            .ok_or_else(|| PlatformWindowError::backend("macos", "failed to create redraw event"))?;
        app.postEvent_atStart(&event, false);
        Ok(())
    }

    fn close(&mut self) -> Result<(), PlatformWindowError> {
        unsafe {
            let _: () = msg_send![&*self.window, close];
        }
        Ok(())
    }
}

/// Create a macOS window with a Metal layer
pub unsafe fn create_window(
    options: &WindowOptions,
    device: &Device,
    mtm: MainThreadMarker,
) -> Result<MacWindow, PlatformWindowError> {
    validate_window_options(options)?;

    // Define window frame
    let frame = NSRect::new(
        NSPoint::new(100.0, 100.0),
        NSSize::new(options.size.width as f64, options.size.height as f64),
    );

    // Window style
    let mut style =
        NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
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
    let _: () = msg_send![&*window, setAcceptsMouseMovedEvents: true];
    let _: bool = msg_send![&*window, makeFirstResponder: &*content_view];

    // Center window on screen
    window.center();

    Ok(MacWindow {
        window,
        metal_layer,
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MacWindowBackend;

impl MacWindowBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures {
            lifecycle: true,
            input_events: true,
            dpi: true,
            resizing: true,
            focus: true,
            clipboard: false,
            renderer_attachment: true,
        }
    }

    pub fn unsupported_clipboard_error(&self) -> PlatformWindowError {
        PlatformWindowError::unsupported("macos", PlatformWindowFeature::Clipboard)
    }
}
