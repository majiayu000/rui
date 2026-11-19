//! Shared data and geometry for the RUI and GPUI workbench workload.

pub(crate) const WIDTH: f32 = 1440.0;
pub(crate) const HEIGHT: f32 = 900.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 288.0;
pub(crate) const PANE_WIDTH: f32 = WIDTH - SIDEBAR_WIDTH;
pub(crate) const ROW_HEIGHT: f32 = 20.0;
pub(crate) const FONT_SIZE: f32 = 13.0;
pub(crate) const SESSIONS: usize = 2_000;
pub(crate) const GROUPS: usize = 8;
pub(crate) const BOUND_TERMINALS: usize = 60;
pub(crate) const VISIBLE_ROWS: usize = 45;
pub(crate) const UPDATED_ROWS: usize = 20;
pub(crate) const SIDEBAR_COLORS: [u32; 2] = [0x252938, 0x292d3e];
pub(crate) const PANE_BG: u32 = 0x1e1e1e;
pub(crate) const TEXT_COLOR: u32 = 0xd4d4d4;

pub(crate) fn session_text(index: usize) -> String {
    let group = index % GROUPS + 1;
    if index < BOUND_TERMINALS {
        format!("Space {group} · Session {index:04} · T{index:02}")
    } else {
        format!("Space {group} · Session {index:04}")
    }
}

pub(crate) fn terminal_text(row: usize, turn: usize) -> String {
    const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    if row < UPDATED_ROWS {
        format!("{turn:08} row {row:02} {}", &HEX[..60])
    } else {
        format!("static   row {row:02} {}", &HEX[..60])
    }
}
