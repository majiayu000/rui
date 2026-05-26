//! Advanced UI controls example.

use rui::advanced_ui as ui;
use rui::prelude::*;

fn main() {
    App::new().run(|cx| {
        // Keep the example command finite; set RUI_ADVANCED_UI_HOLD=1 to inspect it manually.
        if std::env::var_os("RUI_ADVANCED_UI_HOLD").is_none() {
            cx.quit();
        }

        controls_panel()
    });
}

fn controls_panel() -> impl Element {
    ui::container()
        .w(760.0)
        .h(520.0)
        .padding(28.0)
        .background(Color::hex(0xf8fafc))
        .child(
            ui::column()
                .spacing(18.0)
                .child(controls_header())
                .child(action_row())
                .child(setting_row())
                .child(
                    ui::progress_bar(0.68)
                        .width(420.0)
                        .size(ui::ControlSize::Large),
                )
                .child(
                    ui::segmented_control(
                        [
                            ("overview", "Overview"),
                            ("activity", "Activity"),
                            ("alerts", "Alerts"),
                        ],
                        "overview",
                    )
                    .size(ui::ControlSize::Medium),
                )
                .child(
                    ui::scrollable(
                        ui::column()
                            .spacing(8.0)
                            .child(log_row("Renderer boundary ready"))
                            .child(log_row("Text metrics available"))
                            .child(log_row("Stateful views wired"))
                            .child(log_row("Controls layer in progress")),
                    )
                    .w(440.0)
                    .h(128.0)
                    .background(Color::WHITE),
                ),
        )
}

fn controls_header() -> impl Element {
    ui::column()
        .spacing(4.0)
        .child(
            ui::text("Controls")
                .size(24.0)
                .bold()
                .color(Color::hex(0x111827)),
        )
        .child(
            ui::text("Reusable controls built on the advanced UI layer")
                .size(13.0)
                .color(Color::hex(0x6b7280)),
        )
}

fn action_row() -> impl Element {
    ui::row()
        .spacing(10.0)
        .child(ui::button("Save").primary())
        .child(ui::button("Preview").outline())
        .child(ui::tooltip(ui::button("Help").ghost(), "Open docs"))
}

fn setting_row() -> impl Element {
    ui::row()
        .spacing(16.0)
        .cross_axis_alignment(ui::CrossAxisAlignment::Center)
        .child(ui::checkbox("Enable sync").checked(true))
        .child(ui::checkbox("Require review"))
}

fn log_row(label: &'static str) -> impl Element {
    ui::container()
        .w(380.0)
        .h(28.0)
        .padding(6.0)
        .background(Color::hex(0xf3f4f6))
        .radius(4.0)
        .child(ui::text(label).size(12.0).color(Color::hex(0x374151)))
}
