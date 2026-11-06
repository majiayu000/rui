//! Native acceptance: Tab/Shift-Tab, IME composition, accessibility and save/cancel.
//! Run `cargo run --example interaction_acceptance`; changes remain in memory.
use rui::advanced_ui::{Button, Dialog};
use rui::prelude::*;

#[derive(Default)]
struct Editor {
    filter: String,
    draft: String,
    saved: String,
    confirming: bool,
    status: String,
}
impl View for Editor {
    type Element = Div;
    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        let mut root = div()
            .w(600.0)
            .h(520.0)
            .p(24.0)
            .gap(12.0)
            .flex_col()
            .bg(Color::hex(0xf8fafc))
            .child(text("RUI interaction acceptance").size(24.0))
            .child(
                input()
                    .id(ElementId(91001))
                    .accessibility_label("Filter")
                    .placeholder("Filter notes")
                    .value(self.filter.clone())
                    .on_change(cx.text_listener(|this, value, _| this.filter = value)),
            )
            .child(text(
                if "Note".to_lowercase().contains(&self.filter.to_lowercase()) {
                    "Note"
                } else {
                    "No matching notes"
                },
            ))
            .child(
                input()
                    .id(ElementId(91002))
                    .accessibility_label("Draft")
                    .placeholder("Edit note, including Chinese input")
                    .value(self.draft.clone())
                    .on_change(cx.text_listener(|this, value, _| this.draft = value)),
            )
            .child(Button::new("Disabled").id(ElementId(91003)).disabled(true))
            .child(
                Button::new("Save")
                    .id(ElementId(91004))
                    .on_click(cx.listener(|this, _| this.confirming = true)),
            )
            .child(
                Button::new("Revert draft")
                    .id(ElementId(91005))
                    .on_click(cx.listener(|this, _| {
                        this.draft.clone_from(&this.saved);
                        this.status = "Reverted".into();
                    })),
            )
            .child(
                Button::new("Open independent editor")
                    .id(ElementId(91008))
                    .on_click(cx.listener(|_, cx| {
                        cx.app_mut().open_view_window(
                            WindowOptions::default()
                                .title("RUI Independent Editor")
                                .size(640.0, 600.0),
                            Editor::default(),
                        );
                    })),
            )
            .child(text(format!("Saved: {}", self.saved)))
            .child(text(format!("Status: {}", self.status)));
        root = root.child(
            Dialog::new(
                "Confirm save",
                div()
                    .flex_col()
                    .gap(8.0)
                    .child(text("Save the current draft?"))
                    .child(
                        Button::new("Confirm")
                            .id(ElementId(91006))
                            .on_click(cx.listener(|this, _| {
                                this.saved.clone_from(&this.draft);
                                this.confirming = false;
                                this.status = "Saved".into();
                            })),
                    )
                    .child(
                        Button::new("Cancel")
                            .id(ElementId(91007))
                            .on_click(cx.listener(|this, _| {
                                this.confirming = false;
                                this.status = "Canceled".into();
                            })),
                    ),
            )
            .open(self.confirming),
        );
        root
    }
}
fn main() {
    App::new().run_view_with_options(
        WindowOptions::default()
            .title("RUI Interaction Acceptance")
            .size(640.0, 600.0),
        Editor::default(),
    );
}
