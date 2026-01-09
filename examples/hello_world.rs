//! Hello World example for RUI
//!
//! This example demonstrates the basic usage of the RUI framework,
//! showing how to create a simple window with styled elements.

use rui::prelude::*;

fn main() {
    // Create and run the application
    App::new().run(|_cx| {
        // Root container - full window with dark background
        div()
            .w(800.0)
            .h(600.0)
            .bg(Color::hex(0x1a1a2e))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(20.0)
            .child(
                // Header card
                div()
                    .bg(Color::hex(0x16213e))
                    .rounded(16.0)
                    .p(32.0)
                    .shadow_lg()
                    .flex_col()
                    .items_center()
                    .gap(16.0)
                    .child(
                        text("Hello, RUI!")
                            .size(48.0)
                            .bold()
                            .color(Color::hex(0xe94560)),
                    )
                    .child(
                        text("A GPU-accelerated UI framework for Rust")
                            .size(18.0)
                            .color(Color::hex(0xeaeaea)),
                    ),
            )
            .child(
                // Feature cards row
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(feature_card("Fast", "Metal GPU rendering", 0x0f3460))
                    .child(feature_card("Flexible", "Declarative API", 0x533483))
                    .child(feature_card("Modern", "Rust powered", 0xe94560)),
            )
    });
}

/// Create a feature card component
fn feature_card(title: &str, description: &str, color: u32) -> Div {
    div()
        .w(180.0)
        .bg(Color::hex(color))
        .rounded(12.0)
        .p(20.0)
        .flex_col()
        .gap(8.0)
        .child(
            text(title)
                .size(20.0)
                .bold()
                .color(Color::WHITE),
        )
        .child(
            text(description)
                .size(14.0)
                .color(Color::rgba(1.0, 1.0, 1.0, 0.8)),
        )
}
