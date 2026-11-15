//! Counter example - demonstrates stateful view rendering

use rui::prelude::*;

#[derive(Default)]
struct CounterView {
    count: i32,
}

impl View for CounterView {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        div()
            .w(400.0)
            .h(300.0)
            .bg(Color::hex(0x2d3436))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(24.0)
            .child(text("Counter").size(32.0).bold().color(Color::WHITE))
            .child(
                div()
                    .w(200.0)
                    .h(80.0)
                    .bg(Color::hex(0x636e72))
                    .rounded(12.0)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        text(format!("{}", self.count))
                            .size(48.0)
                            .bold()
                            .color(Color::WHITE),
                    ),
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(
                        button("-")
                            .danger()
                            .large()
                            .on_click(cx.listener(|this, _| this.count -= 1)),
                    )
                    .child(
                        button("+")
                            .success()
                            .large()
                            .on_click(cx.listener(|this, _| this.count += 1)),
                    ),
            )
    }
}

fn main() {
    App::new().run_view(CounterView::default());
}
