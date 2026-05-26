//! Counter example - demonstrates stateful view rendering

use rui::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
struct CounterView {
    count: Rc<Cell<i32>>,
}

impl View for CounterView {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let decrement_count = Rc::clone(&self.count);
        let decrement_notify = cx.notifier();
        let increment_count = Rc::clone(&self.count);
        let increment_notify = cx.notifier();

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
                        text(format!("{}", self.count.get()))
                            .size(48.0)
                            .bold()
                            .color(Color::WHITE),
                    ),
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(button("-").danger().large().on_click(move || {
                        decrement_count.set(decrement_count.get() - 1);
                        decrement_notify.notify();
                    }))
                    .child(button("+").success().large().on_click(move || {
                        increment_count.set(increment_count.get() + 1);
                        increment_notify.notify();
                    })),
            )
    }
}

fn main() {
    App::new().run_view(CounterView::default());
}
