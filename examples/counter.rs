//! Counter example - demonstrates state management
//!
//! This example shows how to create an interactive counter
//! with increment/decrement buttons.

use rui::prelude::*;

fn main() {
    App::new().run(|cx| {
        // Create state
        let count = 0i32;

        // Build UI
        div()
            .w(400.0)
            .h(300.0)
            .bg(Color::hex(0x2d3436))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(24.0)
            .child(
                text("Counter")
                    .size(32.0)
                    .bold()
                    .color(Color::WHITE),
            )
            .child(
                // Counter display
                div()
                    .w(200.0)
                    .h(80.0)
                    .bg(Color::hex(0x636e72))
                    .rounded(12.0)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        text(format!("{}", count))
                            .size(48.0)
                            .bold()
                            .color(Color::WHITE),
                    ),
            )
            .child(
                // Buttons row
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(button("-", Color::hex(0xd63031)))
                    .child(button("+", Color::hex(0x00b894))),
            )
    });
}

/// Create a button component
fn button(label: &str, bg_color: Color) -> Div {
    div()
        .w(60.0)
        .h(60.0)
        .bg(bg_color)
        .rounded(30.0)
        .flex()
        .items_center()
        .justify_center()
        .shadow_md()
        .child(
            text(label)
                .size(32.0)
                .bold()
                .color(Color::WHITE),
        )
}
