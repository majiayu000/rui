//! The probe window: one text field per IME scenario, a text area, Markdown
//! and the controls from `examples/controls.rs`.

use rui::advanced_ui as ui;
use rui::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

pub const COMMIT: &str = "Pinyin commit";
pub const ESCAPE: &str = "Pinyin escape";
pub const BACKSPACE: &str = "Pinyin backspace";
pub const SHIFT: &str = "Pinyin shift";
pub const WORDS: &str = "Pinyin words";
pub const DIRECT: &str = "Pinyin direct";
pub const EMULATED: &str = "Emulated IME";
pub const NOTES: &str = "Pinyin notes";
pub const INPUTS: [&str; 7] = [COMMIT, ESCAPE, BACKSPACE, SHIFT, WORDS, DIRECT, EMULATED];

thread_local! {
    /// Each field's value as its `on_change` last reported it.
    static VALUES: RefCell<HashMap<&'static str, String>> = RefCell::new(HashMap::new());
    /// Number of times the Save button ran its handler.
    static SAVES: RefCell<u32> = const { RefCell::new(0) };
}

pub fn value(field: &str) -> String {
    VALUES.with(|values| values.borrow().get(field).cloned().unwrap_or_default())
}

pub fn saves() -> u32 {
    SAVES.with(|saves| *saves.borrow())
}

pub struct ProbeView {
    notifications: bool,
    remember: bool,
    density: String,
    volume: f32,
    fruit: String,
    tab: String,
    due: Option<ui::CalendarDate>,
    toaster: ui::Toaster,
}

impl Default for ProbeView {
    fn default() -> Self {
        Self {
            notifications: true,
            remember: false,
            density: "comfortable".to_string(),
            volume: 40.0,
            fruit: "banana".to_string(),
            tab: "general".to_string(),
            due: None,
            toaster: ui::Toaster::new(),
        }
    }
}

impl View for ProbeView {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let mut fields = div().w(440.0).flex_col().gap(8.0).p(16.0);
        for label in INPUTS {
            let notifier = cx.notifier();
            fields = fields.child(
                input()
                    .w(400.0)
                    .accessibility_label(label)
                    .placeholder(label)
                    .value(value(label))
                    .on_change(move |text| {
                        VALUES.with(|values| values.borrow_mut().insert(label, text.to_string()));
                        notifier.notify();
                    }),
            );
        }
        let notifier = cx.notifier();
        fields = fields
            .child(
                text_area()
                    .w(400.0)
                    .h(90.0)
                    .accessibility_label(NOTES)
                    .placeholder(NOTES)
                    .value(value(NOTES))
                    .on_change(move |text| {
                        VALUES.with(|values| values.borrow_mut().insert(NOTES, text.to_string()));
                        notifier.notify();
                    }),
            )
            .child(markdown(
                "**Markdown** with `code` and a [link](https://example.com).\n\n- first item\n- second item",
            ));

        let controls =
            div()
                .w(480.0)
                .flex_col()
                .gap(10.0)
                .p(16.0)
                .child(
                    ui::switch("Notifications")
                        .on(self.notifications)
                        .on_change(cx.listener_with(|this, on, _| this.notifications = on)),
                )
                .child(
                    ui::checkbox("Remember me")
                        .checked(self.remember)
                        .on_change(cx.listener_with(|this, on, _| this.remember = on)),
                )
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
                )
                .child(
                    ui::slider("Volume", self.volume)
                        .step(5.0)
                        .width(240.0)
                        .on_change(cx.listener_with(|this, value, _| this.volume = value)),
                )
                .child(
                    ui::select(
                        "Fruit",
                        [
                            ("apple", "Apple"),
                            ("banana", "Banana"),
                            ("cherry", "Cherry"),
                        ],
                        self.fruit.clone(),
                    )
                    .on_change(cx.text_listener(|this, value, _| this.fruit = value)),
                )
                .child(
                    ui::tabs(
                        [("general", "General"), ("advanced", "Advanced")],
                        self.tab.clone(),
                    )
                    .accessibility_label("Sections")
                    .panel(ui::tab_panel("general", text("General settings")))
                    .panel(ui::tab_panel("advanced", text("Advanced settings")))
                    .on_change(cx.text_listener(|this, value, _| this.tab = value)),
                )
                .child(ui::date_picker("Due date", self.due).on_change(
                    cx.listener_with(|this, day: ui::CalendarDate, _| this.due = Some(day)),
                ))
                .child(save_button(&self.toaster))
                .child(self.toaster.render(cx));

        div()
            .w_full()
            .h_full()
            .bg(Color::WHITE)
            .flex_row()
            .child(fields)
            .child(controls)
    }
}

fn save_button(toaster: &ui::Toaster) -> ui::Button {
    let toaster = toaster.clone();
    ui::button("Save preferences").on_click(move || {
        SAVES.with(|saves| *saves.borrow_mut() += 1);
        toaster.show(ui::Toast::success("Preferences saved"));
    })
}
