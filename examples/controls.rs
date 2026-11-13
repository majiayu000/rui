//! Slider, select, radio group, switch and splitter, each bound to view state.
//!
//! Run with `cargo run --example controls`. Drag the bar between the panes,
//! or Tab to a control and use the keyboard.

use rui::advanced_ui as ui;
use rui::prelude::*;

pub struct ControlsView {
    pub sidebar: f32,
    pub notifications: bool,
    pub density: String,
    pub volume: f32,
    pub fruit: String,
}

impl Default for ControlsView {
    fn default() -> Self {
        Self {
            sidebar: 240.0,
            notifications: true,
            density: "comfortable".to_string(),
            volume: 40.0,
            fruit: "banana".to_string(),
        }
    }
}

fn heading(content: impl Into<String>) -> Text {
    text(content.into()).size(13.0).color(Color::hex(0x6b7280))
}

impl View for ControlsView {
    type Element = ui::Splitter;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let sidebar = div()
            .w_full()
            .h_full()
            .p(16.0)
            .flex_col()
            .gap(12.0)
            .bg(Color::hex(0xf3f4f6))
            .child(heading("Preferences"))
            .child(
                ui::switch("Notifications")
                    .on(self.notifications)
                    .on_change(cx.listener_with(|this, on, _| this.notifications = on)),
            )
            .child(heading("Density"))
            .child(
                ui::radio_group(
                    "Density",
                    [
                        ("compact", "Compact"),
                        ("comfortable", "Comfortable"),
                        ("spacious", "Spacious"),
                    ],
                    self.density.clone(),
                )
                .on_change(cx.text_listener(|this, value, _| this.density = value)),
            );

        let main = div()
            .w_full()
            .h_full()
            .p(16.0)
            .flex_col()
            .gap(12.0)
            .bg(Color::WHITE)
            .child(heading(format!("Volume: {}", self.volume)))
            .child(
                ui::slider("Volume", self.volume)
                    .step(5.0)
                    .width(240.0)
                    .disabled(!self.notifications)
                    .on_change(cx.listener_with(|this, value, _| this.volume = value)),
            )
            .child(heading(format!("Fruit: {}", self.fruit)))
            .child(
                ui::select(
                    "Fruit",
                    [
                        ("apple", "Apple"),
                        ("banana", "Banana"),
                        ("blueberry", "Blueberry"),
                        ("cherry", "Cherry"),
                    ],
                    self.fruit.clone(),
                )
                .on_change(cx.text_listener(|this, value, _| this.fruit = value)),
            );

        ui::splitter("Sidebar", sidebar, main)
            .position(self.sidebar)
            .limits(160.0, 420.0)
            .on_change(cx.listener_with(|this, at, _| this.sidebar = at))
    }
}

fn main() {
    App::new().run_view(ControlsView::default());
}
