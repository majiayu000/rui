//! Animation demo - showcases the animation system

use rui::prelude::*;

fn main() {
    App::new().run(|_cx| {
        div()
            .w(800.0)
            .h(600.0)
            .bg(Color::hex(0x0f172a))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(32.0)
            // Title
            .child(
                text("Animation Easing Functions")
                    .size(32.0)
                    .bold()
                    .color(Color::WHITE),
            )
            // Easing demo cards
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(easing_card("Linear", Easing::Linear))
                    .child(easing_card("Ease In", Easing::EaseIn))
                    .child(easing_card("Ease Out", Easing::EaseOut))
                    .child(easing_card("Ease In Out", Easing::EaseInOut)),
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(easing_card("Bounce", Easing::EaseOutBounce))
                    .child(easing_card("Elastic", Easing::EaseOutElastic))
                    .child(easing_card("Back", Easing::EaseOutBack))
                    .child(easing_card("Expo", Easing::EaseOutExpo)),
            )
            // Transform demo
            .child(
                div()
                    .flex_col()
                    .items_center()
                    .gap(16.0)
                    .mt(24.0)
                    .child(
                        text("Transforms")
                            .size(24.0)
                            .semibold()
                            .color(Color::WHITE),
                    )
                    .child(
                        div()
                            .flex_row()
                            .gap(24.0)
                            .child(transform_demo("Rotate", Color::hex(0xef4444)))
                            .child(transform_demo("Scale", Color::hex(0x22c55e)))
                            .child(transform_demo("Translate", Color::hex(0x3b82f6))),
                    ),
            )
    });
}

/// Easing function visualization card
fn easing_card(name: &str, _easing: Easing) -> Div {
    div()
        .w(160.0)
        .bg(Color::hex(0x1e293b))
        .rounded(12.0)
        .p(16.0)
        .flex_col()
        .items_center()
        .gap(12.0)
        // Curve visualization area
        .child(
            div()
                .w(120.0)
                .h(80.0)
                .bg(Color::hex(0x334155))
                .rounded(8.0)
                .flex()
                .items_end()
                .justify_center()
                .p(8.0)
                // Animated ball
                .child(
                    div()
                        .w(16.0)
                        .h(16.0)
                        .rounded_full()
                        .bg(Color::hex(0x6366f1))
                        .shadow_md(),
                ),
        )
        .child(text(name).size(14.0).color(Color::hex(0x94a3b8)))
}

/// Transform demonstration
fn transform_demo(name: &str, color: Color) -> Div {
    div()
        .flex_col()
        .items_center()
        .gap(8.0)
        .child(
            div()
                .w(80.0)
                .h(80.0)
                .bg(color)
                .rounded(12.0)
                .shadow_lg()
                .flex()
                .items_center()
                .justify_center()
                .child(text(name.chars().next().unwrap().to_string()).size(24.0).bold().color(Color::WHITE)),
        )
        .child(text(name).size(12.0).color(Color::hex(0x94a3b8)))
}

trait DivExt {
    fn mt(self, margin: f32) -> Self;
}

impl DivExt for Div {
    fn mt(mut self, margin: f32) -> Self {
        // Note: In a full implementation, we'd have margin-top style
        self.my(margin / 2.0)
    }
}
