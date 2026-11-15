# Getting started with state and events

RUI currently runs native windows on macOS with Metal. Use Rust 1.88 or later
and a local desktop session to try the UI. Headless tests can exercise state and
event routing without opening a window; they do not verify native rendering.

## Run the counter

From a checkout of this repository:

```sh
git clone https://github.com/majiayu000/rui.git
cd rui
cargo run --example counter
```

Click `+` and `-`; the number should change on the following frame. The complete
[counter example](../examples/counter.rs) keeps its count as an ordinary `i32`.
It does not need shared cells or a notifier for each button.

## Build your own view

Create a binary project with `cargo new my-counter`. In its `Cargo.toml`, add the
Git dependency below (the crates.io package named `rui` is a different project):

```toml
[dependencies]
rui = { git = "https://github.com/majiayu000/rui", rev = "1311a1d6b98a601de603d215ebc1d21f408fd7a3" }
```

This is the same source baseline as the README and includes the listener APIs below; it is not a release tag.
Replace `src/main.rs` with this complete program and run `cargo run`:

```rust
use rui::prelude::*;

#[derive(Default)]
struct Counter {
    count: i32,
}

impl View for Counter {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        div()
            .w(320.0)
            .h(180.0)
            .flex_col()
            .gap(12.0)
            .child(text(format!("Count: {}", self.count)))
            .child(button("+").on_click(cx.listener(|this, _| this.count += 1)))
    }
}

fn main() {
    App::new().run_view(Counter::default());
}
```

`render` builds an element tree from the view's current state. `cx.listener`
returns the callback expected by `on_click`. When the event fires, it queues your
closure. Before the owning view renders again, the closure receives `&mut Self`
and `&mut ViewContext<Self>`, then the view renders with the updated state.
No explicit `notify()` is needed for that owning view. Do not rely on the mutation
having happened synchronously when the raw callback returns.

## Text and other event values

For a view with a `draft: String` field, bind the current value and update it with
`text_listener` inside `render`:

```rust,ignore
input()
    .id(ElementId(100))
    .accessibility_label("Draft")
    .value(self.draft.clone())
    .on_change(cx.text_listener(|this, value, _| this.draft = value))
```

Keep the input ID stable between renders. `text_listener` copies the event's
borrowed `&str` into a `String` so it can reach the queued closure safely. It also
works with `on_submit`; both callbacks receive the text as their second argument.
For an owned event value, use `listener_with`, for example a checkbox's boolean:

```rust,ignore
rui::advanced_ui::Checkbox::new("Sync")
    .checked(self.checked)
    .on_change(cx.listener_with(|this, checked, _| this.checked = checked))
```

The callback context reaches other entities. `Entity::update` changes one and
notifies it, so every view that read it renders again. For example, with an
existing `Entity<String>` in `this.saved`:

```rust,ignore
cx.text_listener(|this, value, cx| {
    this.saved.update(cx, |saved, _| *saved = value);
})
```

`update` returns `None` once the entity was released. Handle that according to
your application's lifetime rules rather than assuming every handle is live.

## Async work

`cx.spawn` runs a task on the UI thread with a handle on the view. Slow work
goes to a background thread with `background_spawn`; the result goes back into
the view with `this.update`, and the view renders again without a `notify`:

```rust,ignore
cx.spawn(|this, cx| async move {
    let body = cx.background_spawn(async move { fetch(url) }).await;
    this.update(&cx, |this, _| this.body = body).ok();
})
.detach();
```

The handle does not keep the view alive: `update` returns `Err(ViewReleased)`
once it is gone. The [spawn example](../examples/spawn_reply.rs) streams a reply
this way.

## Key bindings

A key bound to an action in the app's keymap reaches the focused element, then
the divs around it, then the rest of the tree. `Div::on_action` handles one in
the view that renders it:

```rust,ignore
// Once, at startup:
cx.keymap_mut().bind(KeyCode::N, Modifiers::meta(), ActionId::custom("todo.add"))?;

// In render:
div().on_action(ActionId::custom("todo.add"), cx.listener(|this, _| this.add()))
```

The [interaction example](../examples/interaction_acceptance.rs) combines text
editing, save/revert, and a confirmation dialog. These adapters use the existing
element callbacks; there is no separate handler-context API to migrate to.

## Check the update path

From the RUI repository root:

```sh
cargo test --no-default-features --test view_listener --test listener_workflow --test view_updates
```

These tests drive pointer and keyboard events, check changed state in the next
frame's accessibility tree, and cover updates to another entity. The workflow
test types text, submits it into an entity, clears the draft, and checks that a
later edit leaves the saved value intact. They do not prove first-time developer
usability, native IME behavior, or what the Metal-rendered window looks like.
