//! Advanced UI layout example.

use rui::advanced_ui as ui;
use rui::prelude::*;

fn main() {
    App::new().run(|cx| {
        // Keep the example command finite; set RUI_ADVANCED_UI_HOLD=1 to inspect it manually.
        if std::env::var_os("RUI_ADVANCED_UI_HOLD").is_none() {
            cx.quit();
        }

        dashboard()
    });
}

fn dashboard() -> impl Element {
    ui::container()
        .w(920.0)
        .h(620.0)
        .padding(28.0)
        .background(Color::hex(0xf5f7fb))
        .child(
            ui::column()
                .spacing(20.0)
                .child(page_header())
                .child(
                    ui::row()
                        .spacing(16.0)
                        .child(metric_card("Revenue", "$42.8k", "12.4%", 0x2563eb))
                        .child(metric_card("Usage", "18.2k", "8.1%", 0x059669))
                        .child(metric_card("Latency", "86ms", "4.7%", 0xdc2626)),
                )
                .child(work_area()),
        )
}

fn page_header() -> impl Element {
    ui::row()
        .h(72.0)
        .padding(18.0)
        .background(Color::WHITE)
        .radius(8.0)
        .main_axis_alignment(ui::MainAxisAlignment::SpaceBetween)
        .cross_axis_alignment(ui::CrossAxisAlignment::Center)
        .child(
            ui::column()
                .spacing(4.0)
                .child(
                    ui::text("Operations")
                        .size(24.0)
                        .bold()
                        .color(Color::hex(0x111827)),
                )
                .child(
                    ui::text("Regional systems status")
                        .size(13.0)
                        .color(Color::hex(0x6b7280)),
                ),
        )
        .child(
            ui::container()
                .padding(10.0)
                .background(Color::hex(0xe0f2fe))
                .radius(6.0)
                .child(
                    ui::text("Live")
                        .size(13.0)
                        .semibold()
                        .color(Color::hex(0x0369a1)),
                ),
        )
}

fn metric_card(
    label: &'static str,
    value: &'static str,
    delta: &'static str,
    accent: u32,
) -> impl Element {
    ui::container()
        .flex_grow(1.0)
        .padding(18.0)
        .background(Color::WHITE)
        .radius(8.0)
        .child(
            ui::column()
                .spacing(10.0)
                .child(
                    ui::text(label)
                        .size(13.0)
                        .medium()
                        .color(Color::hex(0x6b7280)),
                )
                .child(
                    ui::text(value)
                        .size(28.0)
                        .bold()
                        .color(Color::hex(0x111827)),
                )
                .child(
                    ui::text(delta)
                        .size(13.0)
                        .semibold()
                        .color(Color::hex(accent)),
                ),
        )
}

fn work_area() -> impl Element {
    ui::row()
        .h(320.0)
        .spacing(16.0)
        .child(
            ui::container()
                .w(280.0)
                .padding(18.0)
                .background(Color::WHITE)
                .radius(8.0)
                .child(
                    ui::column()
                        .spacing(12.0)
                        .child(ui::text("Queue").size(18.0).bold().color(Color::hex(0x111827)))
                        .child(status_row("Design review", "Ready"))
                        .child(status_row("Renderer split", "Next"))
                        .child(status_row("Text layout", "Planned")),
                ),
        )
        .child(
            ui::container()
                .flex_grow(1.0)
                .padding(18.0)
                .background(Color::WHITE)
                .radius(8.0)
                .child(
                    ui::column()
                        .spacing(14.0)
                        .child(ui::text("Timeline").size(18.0).bold().color(Color::hex(0x111827)))
                        .child(ui::text("North region capacity is holding steady across priority workloads.").size(14.0).color(Color::hex(0x4b5563)))
                        .child(ui::text("Queue pressure remains concentrated in the renderer split lane.").size(14.0).color(Color::hex(0x4b5563))),
                ),
        )
}

fn status_row(label: &'static str, value: &'static str) -> impl Element {
    ui::row()
        .h(36.0)
        .main_axis_alignment(ui::MainAxisAlignment::SpaceBetween)
        .cross_axis_alignment(ui::CrossAxisAlignment::Center)
        .child(ui::text(label).size(14.0).color(Color::hex(0x374151)))
        .child(
            ui::text(value)
                .size(13.0)
                .semibold()
                .color(Color::hex(0x2563eb)),
        )
}
