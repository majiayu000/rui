//! Glyphs of bitmap color fonts, such as emoji.

use super::TextError;
use crate::renderer::resources::{RendererResourceError, RendererResourceKind};
use skrifa::GlyphId;
use skrifa::font::glyph::bitmap::{Origin, Pixels};

/// The bitmap of `glyph_id` at `device_font_size`, for fonts whose glyphs are
/// images (`sbix`, `CBDT`): its offset from the pen position
/// on the baseline, its size and its sRGB RGBA pixels with straight alpha.
/// `None` when the font has no image for the glyph. The strike closest above
/// the size is scaled down to it, in premultiplied alpha so transparent pixels
/// do not darken the edges.
#[allow(clippy::type_complexity)]
pub(super) fn color_bitmap(
    font: &crate::renderer::text_shaping::TextShapingFont,
    glyph_id: u16,
    device_font_size: f32,
) -> Result<Option<(i32, i32, u32, u32, Vec<u8>)>, TextError> {
    // Font sizes here are the height from descent to ascent, as outlines and
    // shaping use them. Strikes are chosen by pixels per em.
    if font.height_em() <= 0.0 {
        return Ok(None);
    }
    let em = font.scale_for_pixel_height(device_font_size * f32::from(font.units_per_em()));
    let pixels_per_em = em.ceil().clamp(1.0, f32::from(u16::MAX)) as u16;
    // The strike closest above the size, else the largest.
    let Some(image) = font
        .font()
        .bitmap_strikes()
        .select(Some(f32::from(pixels_per_em)))
        .and_then(|strike| strike.get(GlyphId::new(u32::from(glyph_id))))
    else {
        return Ok(None);
    };
    let invalid = |reason: String| {
        TextError::Resource(RendererResourceError::invalid(
            RendererResourceKind::Glyph,
            reason,
        ))
    };
    let Pixels::Png(data) = image.pixels() else {
        return Err(invalid(format!(
            "glyph {glyph_id} is an uncompressed {:?} bitmap; only PNG bitmaps are drawn",
            image.source()
        )));
    };
    let decoded = image::load_from_memory_with_format(data, image::ImageFormat::Png)
        .map_err(|err| invalid(format!("glyph {glyph_id} bitmap does not decode: {err}")))?
        .to_rgba8();
    let scale = em / f32::from(image.ppem().max(1));
    let width = ((decoded.width() as f32 * scale).round() as u32).max(1);
    let height = ((decoded.height() as f32 * scale).round() as u32).max(1);
    let mut premultiplied = decoded;
    for pixel in premultiplied.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    let mut scaled = image::imageops::resize(
        &premultiplied,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    for pixel in scaled.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            // Fully transparent pixels keep their (zero) color.
            if let Some(straight) = (u16::from(*channel) * 255 + alpha / 2).checked_div(alpha) {
                *channel = straight.min(255) as u8;
            }
        }
    }
    // The image's bottom left sits `x, y` strike pixels from the origin, y up;
    // `CBDT` and `EBDT` give its top left.
    let x = image.inner_x_bearing();
    let y = match image.origin() {
        Origin::BottomLeft => image.inner_y_bearing(),
        Origin::TopLeft => image.inner_y_bearing() - image.height() as i32,
    };
    let left = (x as f32 * scale).round() as i32;
    let top = -((y as f32 * scale).round() as i32) - height as i32;
    Ok(Some((left, top, width, height, scaled.into_raw())))
}
