//! Glyph coverage masks: rusttype, or the read-fonts outline through
//! ab_glyph_rasterizer with the `fontations` feature (issue #369 spike).

use rusttype::{GlyphId, Scale};

#[cfg(feature = "fontations")]
pub(super) use fontations_coverage as coverage;
#[cfg(not(feature = "fontations"))]
pub(super) use rusttype_coverage as coverage;
/// The pixel box of glyph `id` at `size` drawn at `position`, and its coverage
/// row by row.
#[cfg_attr(feature = "fontations", allow(dead_code))]
pub(crate) fn rusttype_coverage(
    font: &crate::renderer::text_shaping::TextShapingFont,
    id: u16,
    size: f32,
    position: rusttype::Point<f32>,
) -> Option<(rusttype::Rect<i32>, Vec<u8>)> {
    let positioned = font
        .font()
        .glyph(GlyphId(id))
        .scaled(Scale::uniform(size))
        .positioned(position);
    let bounds = positioned.pixel_bounding_box()?;
    let width = bounds.width() as u32;
    let height = bounds.height() as u32;
    let mut pixels = vec![0u8; width as usize * height as usize];
    positioned.draw(|x, y, coverage| {
        if x < width && y < height {
            pixels[(y * width + x) as usize] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    });
    Some((bounds, pixels))
}

#[cfg(feature = "fontations")]
pub(crate) fn fontations_coverage(
    font: &crate::renderer::text_shaping::TextShapingFont,
    id: u16,
    size: f32,
    position: rusttype::Point<f32>,
) -> Option<(rusttype::Rect<i32>, Vec<u8>)> {
    let Some(bounds) = font.pixel_bounds(id, size, position) else {
        #[cfg(test)]
        if rusttype_coverage(font, id, size, position).is_some() {
            eprintln!(
                "COVERAGE DIFF {} glyph {id}: no fontations box",
                font.family
            );
            COVERAGE_DIFFS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        return None;
    };
    let width = bounds.width() as u32;
    let height = bounds.height() as u32;
    // As rusttype scales for `Scale::uniform(size)` and offsets by the box.
    let scale_y = font.font().scale_for_pixel_height(size);
    let scale = (scale_y * size / size, scale_y);
    let offset = (
        position.x - bounds.min.x as f32,
        position.y - bounds.min.y as f32,
    );
    let mut pixels = vec![0u8; width as usize * height as usize];
    font.fontations
        .draw(id, scale, offset, (width, height), |x, y, coverage| {
            if x < width && y < height {
                pixels[(y * width + x) as usize] = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
    #[cfg(test)]
    {
        let reference = rusttype_coverage(font, id, size, position);
        if reference.as_ref().map(|(b, p)| (b.min, b.max, p))
            != Some((bounds.min, bounds.max, &pixels))
        {
            eprintln!(
                "COVERAGE DIFF {} glyph {id} size {size} at {position:?}: rusttype {:?} fontations {:?}",
                font.family,
                reference.as_ref().map(|(b, _)| b),
                bounds
            );
            COVERAGE_DIFFS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    Some((bounds, pixels))
}

#[cfg(all(test, feature = "fontations"))]
pub(crate) static COVERAGE_DIFFS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
