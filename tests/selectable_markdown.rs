//! Selecting and copying a Markdown reply.
#![cfg(feature = "markdown")]

use rui::AppContext;
use rui::advanced_ui::Theme;
use rui::core::event::{Cursor, KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Bounds;
use rui::core::text_editing::ClipboardItem;
use rui::core::{Point, Size};
use rui::elements::element::Element;
use rui::elements::{div, markdown};
use rui::renderer::Primitive;
use rui::testing::{HeadlessSession, mount};
use std::cell::RefCell;
use std::rc::Rc;

const REPLY: &str = "# Answer\n\nUse a [selection scope](https://example.com/scope) around the reply.\n\n- First point\n- Second point\n\n```rust\nlet copied = true;\n```";

fn text_bounds(primitives: &[Primitive], needle: &str) -> Bounds {
    primitives
        .iter()
        .find_map(|primitive| match primitive {
            Primitive::Text {
                content, bounds, ..
            } if content.contains(needle) => Some(*bounds),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{needle} is drawn"))
}

fn middle_of(primitives: &[Primitive], needle: &str) -> Point {
    let bounds = text_bounds(primitives, needle);
    Point::new(
        bounds.x() + bounds.width() * 0.5,
        bounds.y() + bounds.height() * 0.5,
    )
}

fn highlights(primitives: &[Primitive]) -> usize {
    let color = Theme::default().state.selection.to_rgba().to_array();
    primitives
        .iter()
        .filter(|primitive| {
            matches!(primitive, Primitive::Quad { background, .. } if background.to_array() == color)
        })
        .count()
}

fn copy<F, E>(session: &mut HeadlessSession<F, E>) -> Option<String>
where
    F: FnMut(&mut AppContext) -> E,
    E: Element,
{
    session
        .app_context()
        .write_to_clipboard(ClipboardItem::new_string(""))
        .expect("clear clipboard");
    session.dispatch_key_event(&KeyEvent::new(KeyCode::C, Modifiers::meta()));
    session
        .app_context()
        .read_from_clipboard()
        .expect("clipboard")
        .map(|item| item.text().to_string())
        .filter(|text| !text.is_empty())
}

#[test]
fn a_drag_over_a_reply_copies_its_blocks_joined_by_line_breaks() {
    let mut session = mount(Size::new(480.0, 400.0), |_| {
        div().w(480.0).p(12.0).child(markdown(REPLY).selectable())
    })
    .expect("mount");
    let title = text_bounds(session.primitives(), "Answer");
    let from = Point::new(title.x() + 0.5, title.y() + 4.0);
    let to = Point::new(470.0, middle_of(session.primitives(), "copied").y);
    session.pointer_down(from);
    session.pointer_move(to);
    session.pointer_up(to);
    assert_eq!(
        copy(&mut session).as_deref(),
        Some(
            "Answer\nUse a selection scope around the reply.\nFirst point\nSecond point\nlet copied = true;"
        ),
        "list markers are left out"
    );
    assert!(highlights(session.primitives()) >= 5);
}

#[test]
fn a_selection_survives_a_streamed_append() {
    let source = Rc::new(RefCell::new(String::from(
        "First paragraph stays put.\n\nSecond is stream",
    )));
    let shown = source.clone();
    let mut session = mount(Size::new(480.0, 400.0), move |_| {
        div()
            .w(480.0)
            .child(markdown(shown.borrow().clone()).selectable())
    })
    .expect("mount");
    let word = middle_of(session.primitives(), "First paragraph");
    session.pointer_down(word);
    session.pointer_up(word);
    session.pointer_down(word);
    session.pointer_up(word);
    let selected = copy(&mut session).expect("a word is selected");
    assert!(
        "First paragraph stays put.".contains(&selected),
        "{selected}"
    );

    for token in ["ing", " in", ".\n\n- and a list"] {
        source.borrow_mut().push_str(token);
        session.app_context_mut().request_rebuild();
        session.frame().expect("frame");
    }
    assert!(text_bounds(session.primitives(), "and a list").width() > 0.0);
    assert_eq!(copy(&mut session).as_deref(), Some(selected.as_str()));
    assert_eq!(highlights(session.primitives()), 1);

    // A change to the selected text itself drops the selection.
    *source.borrow_mut() = String::from("Rewritten paragraph.");
    session.app_context_mut().request_rebuild();
    session.frame().expect("frame");
    assert_eq!(copy(&mut session), None);
    assert_eq!(highlights(session.primitives()), 0);
}

#[test]
fn a_click_on_a_link_still_opens_it_and_a_drag_over_it_selects() {
    let clicked = Rc::new(RefCell::new(Vec::<String>::new()));
    let sink = clicked.clone();
    let mut session = mount(Size::new(480.0, 200.0), move |_| {
        let sink = sink.clone();
        div().w(480.0).child(
            markdown("Read [the docs](https://example.com) first.")
                .on_link(move |url| sink.borrow_mut().push(url.to_string()))
                .selectable(),
        )
    })
    .expect("mount");
    let link = text_bounds(session.primitives(), "the docs");
    let on_link = Point::new(link.x() + 4.0, link.y() + link.height() * 0.5);
    session.pointer_move(on_link);
    assert_eq!(session.cursor(), Cursor::Pointer);
    let read = middle_of(session.primitives(), "Read");
    session.pointer_move(read);
    assert_eq!(session.cursor(), Cursor::Text);

    session.pointer_down(on_link);
    session.pointer_up(on_link);
    assert_eq!(*clicked.borrow(), ["https://example.com"]);
    assert_eq!(copy(&mut session), None);

    let past = Point::new(link.x() + link.width() + 30.0, on_link.y);
    session.pointer_down(on_link);
    session.pointer_move(past);
    session.pointer_up(past);
    assert_eq!(clicked.borrow().len(), 1, "a drag is no click");
    let selected = copy(&mut session).expect("the drag selects");
    assert!(
        selected.contains("docs") && !selected.contains("Read"),
        "{selected}"
    );
}

#[test]
fn copying_a_table_joins_cells_with_tabs_and_rows_with_line_breaks() {
    let mut session = mount(Size::new(480.0, 300.0), |_| {
        div().w(480.0).child(
            markdown(
                "Totals:\n\n| Name | Count |\n|------|------:|\n| Apples | 3 |\n| Pears | 12 |",
            )
            .selectable(),
        )
    })
    .expect("mount");
    let from = text_bounds(session.primitives(), "Totals");
    session.pointer_down(Point::new(from.x() + 0.5, from.y() + 4.0));
    session.pointer_move(Point::new(475.0, 295.0));
    session.pointer_up(Point::new(475.0, 295.0));
    assert_eq!(
        copy(&mut session).as_deref(),
        Some("Totals:\nName\tCount\nApples\t3\nPears\t12")
    );
}

/// `RUI_SELECTION_PNG=<path>` writes a reply with a selection, drawn by
/// Metal, to `<path>`.
#[cfg(all(target_os = "macos", feature = "metal"))]
#[test]
fn a_selection_renders_through_metal() {
    use rui::renderer::RendererError;
    use rui::renderer::metal::MetalRenderer;

    let size = Size::new(480.0, 300.0);
    let theme = Theme::default();
    let mut session = mount(size, move |_| {
        div()
            .w(480.0)
            .h(300.0)
            .bg(theme.colors.surface)
            .p(16.0)
            .child(
                markdown(format!("{REPLY}\n\nمرحبا بالعالم، النص من اليمين"))
                    .theme(theme)
                    .selectable(),
            )
    })
    .expect("mount");
    let from = middle_of(session.primitives(), "selection scope");
    let to = middle_of(session.primitives(), "مرحبا");
    session.pointer_down(from);
    session.pointer_move(to);
    session.pointer_up(to);
    session.frame().expect("frame");
    assert!(highlights(session.primitives()) >= 3);
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer: {err}"),
    };
    let pixels = renderer
        .capture_frame_pixels_at_scale(session.scene(), size, 2.0)
        .expect("capture");
    if let Ok(path) = std::env::var("RUI_SELECTION_PNG") {
        image::RgbaImage::from_raw(960, 600, pixels)
            .expect("rgba")
            .save(path)
            .expect("png");
    }
}
