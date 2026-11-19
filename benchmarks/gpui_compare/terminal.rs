//! `terminal_rows` on GPUI: RUI's `benches/compare_terminal`. One `canvas` paints a
//! terminal's 45 rows of SF Mono 13 px, one shaped line per row, as a terminal pane
//! does. Each frame rows 0..20 get new text and the other 25 keep theirs.
//! `run.sh` copies this file next to `rui_compare.rs`.

use gpui::{
    BenchAppContext, Context, IntoElement, Render, SharedString, Styled, TextAlign, TextRun,
    Window, canvas, fill, font, point, px, rgb,
};

const ROWS: usize = 45;
const CHANGED_ROWS: usize = 20;
const LINE_HEIGHT: f32 = 20.;
const FONT_SIZE: f32 = 13.;
const WIDTH: f32 = 1440.;
const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn row_text(row: usize, turn: usize) -> String {
    if row < CHANGED_ROWS {
        format!("{turn:08} row {row:02} {}", &HEX[..60])
    } else {
        format!("static   row {row:02} {}", &HEX[..60])
    }
}

pub(crate) struct Terminal {
    pub(crate) turn: usize,
}

impl Terminal {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        super::load_sf_mono(cx.text_system());
        Self { turn: 0 }
    }
}

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<SharedString> = (0..ROWS)
            .map(|row| row_text(row, self.turn).into())
            .collect();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                window.paint_quad(fill(bounds, rgb(0x1e1e1e)));
                for (index, text) in rows.iter().enumerate() {
                    let run = TextRun {
                        len: text.len(),
                        font: font(super::EDITOR_FONT),
                        color: rgb(0xd4d4d4).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line =
                        window
                            .text_system()
                            .shape_line(text.clone(), px(FONT_SIZE), &[run], None);
                    let origin = point(
                        bounds.origin.x,
                        bounds.origin.y + px(index as f32 * LINE_HEIGHT),
                    );
                    line.paint(origin, px(LINE_HEIGHT), TextAlign::Left, None, window, cx)
                        .expect("paint a terminal row");
                }
            },
        )
        .w(px(WIDTH))
        .h(px(ROWS as f32 * LINE_HEIGHT))
    }
}

pub(crate) fn terminal_rows(cx: &mut BenchAppContext) {
    super::bench_view(cx, "terminal_rows", Terminal::new, |terminal, _, cx| {
        terminal.turn += 1;
        cx.notify();
    });
}
