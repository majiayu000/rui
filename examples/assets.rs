//! Asset gallery: images in every fit mode, an image decoded off the UI thread,
//! and a row of SVG icons.
//!
//! The photo is encoded to PNG at startup and handed to `cx.image`, which
//! decodes it on a background thread; the placeholder shows until it lands,
//! and then only the photo is painted again. A missing file and an unsupported
//! icon show their errors in place.
//!
//! `RUI_ASSETS_CHECK=1` reports how long the decode took to reach a frame and
//! quits once it has.

use rui::prelude::*;
use rui::{ImageAsset, ImageState, svg};
use std::io::Cursor;
use std::sync::Arc;
use std::time::Instant;

const BOX: f32 = 112.0;

const FITS: [(ImageFit, &str); 5] = [
    (ImageFit::Fill, "fill"),
    (ImageFit::Contain, "contain"),
    (ImageFit::Cover, "cover"),
    (ImageFit::None, "none"),
    (ImageFit::ScaleDown, "scale down"),
];

const ICONS: [(&str, u32); 6] = [
    (
        r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14M12 5v14"/></svg>"#,
        0x60a5fa,
    ),
    (
        r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></svg>"#,
        0xf472b6,
    ),
    (
        r#"<svg viewBox="0 0 24 24"><path d="M12 2l3 7h7l-5.5 4.5 2 7.5-6.5-4.5-6.5 4.5 2-7.5L2 9h7z"/></svg>"#,
        0xfacc15,
    ),
    (
        r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/></svg>"#,
        0x34d399,
    ),
    (
        r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 12 10 18 20 6"/></svg>"#,
        0xa78bfa,
    ),
    (
        r#"<svg viewBox="0 0 24 24"><path d="M12 21C5 15 2 11.5 2 8a5 5 0 0 1 10-1 5 5 0 0 1 10 1c0 3.5-3 7-10 13z"/></svg>"#,
        0xf87171,
    ),
];

/// Uses a gradient, which the subset does not draw: shows its error.
const UNSUPPORTED: &str =
    r##"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="url(#glow)"/></svg>"##;

/// A `width` by `height` picture: a sky, a sun and hills.
pub fn picture(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let hill = 0.62 + 0.08 * (u * 9.0).sin() + 0.05 * (u * 23.0).cos();
            let sun = ((u - 0.72).powi(2) + (v - 0.3).powi(2)).sqrt() < 0.12;
            let [r, g, b] = if v > hill {
                [40.0 + 60.0 * v, 120.0 + 60.0 * (1.0 - v), 70.0]
            } else if sun {
                [255.0, 214.0, 102.0]
            } else {
                [70.0 + 90.0 * v, 130.0 + 80.0 * v, 230.0]
            };
            pixels.extend([r as u8, g as u8, b as u8, 255]);
        }
    }
    pixels
}

fn png(width: u32, height: u32) -> Arc<[u8]> {
    let image = image::RgbaImage::from_raw(width, height, picture(width, height))
        .expect("the picture fills the image");
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode png");
    bytes.into_inner().into()
}

pub struct Gallery {
    /// Shown in every fit mode: wider than tall, and larger than a box.
    pixels: Vec<u8>,
    /// Encoded once, so `cx.image` finds the same asset every render.
    photo: ImageAsset,
    started: Instant,
    check: bool,
    reported: bool,
}

impl Gallery {
    pub fn new() -> Self {
        Self {
            pixels: picture(160, 90),
            photo: ImageAsset::Bytes(png(2048, 1152)),
            started: Instant::now(),
            check: std::env::var_os("RUI_ASSETS_CHECK").is_some(),
            reported: false,
        }
    }
}

impl Default for Gallery {
    fn default() -> Self {
        Self::new()
    }
}

fn heading(label: &str) -> Text {
    text(label).size(15.0).bold().color(Color::hex(0xe2e8f0))
}

fn caption(label: &str) -> Text {
    text(label).size(12.0).color(Color::hex(0x94a3b8))
}

fn tile(image: Image, label: &str) -> Div {
    div()
        .flex_col()
        .gap(6.0)
        .child(image.size(Size::new(BOX, BOX)))
        .child(caption(label))
}

impl View for Gallery {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        let photo = cx.image(self.photo.clone());
        if self.check && !self.reported && photo.state() != ImageState::Loading {
            self.reported = true;
            eprintln!(
                "assets: 2048 x 1152 photo {:?} after {:?}, on a background thread",
                photo.state(),
                self.started.elapsed()
            );
            cx.app_mut().quit();
        }
        let missing = cx.image("does/not/exist.png");

        let fits = FITS.map(|(fit, label)| {
            let image = Image::from_data(self.pixels.clone(), 160, 90).fit(fit);
            tile(image.placeholder(Color::hex(0x334155)), label)
        });
        let icons = ICONS.map(|(markup, color)| {
            div().p(10.0).bg(Color::hex(0x1e293b)).rounded(8.0).child(
                svg(markup)
                    .size(Size::new(28.0, 28.0))
                    .color(Color::hex(color)),
            )
        });

        div()
            .w_full()
            .h_full()
            .bg(Color::hex(0x0f172a))
            .p(24.0)
            .flex_col()
            .gap(14.0)
            .child(heading("Fit modes: a 160 x 90 picture in 112 x 112 boxes"))
            .child(div().flex_row().gap(14.0).children(fits))
            .child(heading("Decoded off the UI thread"))
            .child(
                div()
                    .flex_row()
                    .gap(14.0)
                    .child(tile(
                        photo.cover().placeholder(Color::hex(0x334155)),
                        "2048 x 1152 PNG",
                    ))
                    .child(tile(missing.alt("no such file"), "a missing file")),
            )
            .child(heading("SVG icons, tinted"))
            .child(
                div()
                    .flex_row()
                    .gap(10.0)
                    .children(icons)
                    .child(svg(UNSUPPORTED).size(Size::new(420.0, 48.0))),
            )
    }
}

#[allow(dead_code)]
fn main() {
    App::new().run_view(Gallery::new());
}
