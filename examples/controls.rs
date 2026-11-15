//! Slider, select, radio group, switch, splitter and date picker, each bound
//! to view state, with toasts confirming changes.
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
    pub due: Option<ui::CalendarDate>,
    pub toaster: ui::Toaster,
}

impl Default for ControlsView {
    fn default() -> Self {
        Self {
            sidebar: 240.0,
            notifications: true,
            density: "comfortable".to_string(),
            volume: 40.0,
            fruit: "banana".to_string(),
            due: None,
            toaster: ui::Toaster::new(),
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
            )
            .child(heading("Due date (weekdays only)"))
            .child(
                ui::date_picker("Due date", self.due)
                    .disabled_dates(|day| {
                        matches!(day.weekday(), ui::Weekday::Saturday | ui::Weekday::Sunday)
                    })
                    .on_change(cx.listener_with(|this, day: ui::CalendarDate, _| {
                        this.due = Some(day);
                        this.toaster
                            .show(ui::Toast::info("Due date set").description(day.long_label()));
                    })),
            )
            .child(save_button(&self.toaster))
            .child(self.toaster.render(cx));

        ui::splitter("Sidebar", sidebar, main)
            .position(self.sidebar)
            .limits(160.0, 420.0)
            .on_change(cx.listener_with(|this, at, _| this.sidebar = at))
    }
}

fn save_button(toaster: &ui::Toaster) -> ui::Button {
    let toaster = toaster.clone();
    ui::button("Save preferences").on_click(move || {
        let undo = toaster.clone();
        toaster.show(
            ui::Toast::success("Preferences saved")
                .description("They apply to every window.")
                .action("Undo", move || {
                    undo.show(ui::Toast::warning("Changes undone"));
                }),
        );
    })
}

fn main() {
    App::new().run_view(ControlsView::default());
}
