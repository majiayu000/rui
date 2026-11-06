//! `cargo run --example variable_list`: scroll mixed-height wrapped log rows.
use rui::prelude::*;
fn main() {
    let handle = VariableListHandle::new();
    App::new().run(move |_| {
        variable_list(&handle, (0..100_000).map(|i| ElementId::from(i + 1u64)), |id| {
            let repeats = 1 + id.0 as usize % 5;
            div().flex_col().p(8.0)
                .bg(if id.0 % 2 == 0 { Color::hex(0x182030) } else { Color::hex(0x222b3b) })
                .child(text(format!("Log {}: {}", id.0, "Wrapped text keeps this item's identity while its measured height changes. ".repeat(repeats))).wrap().color(Color::WHITE))
        }).w(600.0).h(500.0)
    });
}
