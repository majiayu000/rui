//! `terminal_rows` on GPUI: RUI's `benches/compare_terminal`. One `canvas` paints a
//! terminal's 45 rows of SF Mono 13 px, one shaped line per row, as a terminal pane
//! does. Each frame rows 0..20 get new text and the other 25 keep theirs.
//! `run.sh` copies this file next to `rui_compare.rs`.

use gpui::{
    BenchAppContext, Context, IntoElement, Render, SharedString, Styled, TextAlign, TextRun,
    Window, canvas, fill, font, point, px, rgb,
};

use super::workbench_data::{
    FONT_SIZE, PANE_BG, ROW_HEIGHT, TEXT_COLOR, VISIBLE_ROWS, WIDTH, terminal_text,
};

pub(crate) struct Terminal {
    pub(crate) turn: usize,
    width: f32,
}

impl Terminal {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self::with_width(cx, WIDTH)
    }

    pub(crate) fn with_width(cx: &mut Context<Self>, width: f32) -> Self {
        super::load_sf_mono(cx.text_system());
        Self { turn: 0, width }
    }
}

impl Render for Terminal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<SharedString> = (0..VISIBLE_ROWS)
            .map(|row| terminal_text(row, self.turn).into())
            .collect();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                window.paint_quad(fill(bounds, rgb(PANE_BG)));
                for (index, text) in rows.iter().enumerate() {
                    let run = TextRun {
                        len: text.len(),
                        font: font(super::EDITOR_FONT),
                        color: rgb(TEXT_COLOR).into(),
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
                        bounds.origin.y + px(index as f32 * ROW_HEIGHT),
                    );
                    line.paint(origin, px(ROW_HEIGHT), TextAlign::Left, None, window, cx)
                        .expect("paint a terminal row");
                }
            },
        )
        .w(px(self.width))
        .h(px(VISIBLE_ROWS as f32 * ROW_HEIGHT))
    }
}

pub(crate) fn terminal_rows(cx: &mut BenchAppContext, sample_gpu: bool) {
    super::bench_view(
        cx,
        "terminal_rows",
        sample_gpu,
        Terminal::new,
        |terminal, _, cx| {
            terminal.turn += 1;
            cx.notify();
        },
    );
}
