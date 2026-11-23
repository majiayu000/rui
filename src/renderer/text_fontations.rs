//! Issue #369 spike (feature `fontations`): shaping through HarfRust and glyph
//! boxes and coverage through the read-fonts font model that Skrifa and
//! HarfRust share. Metrics, fallback coverage, names and color bitmaps still go
//! through rusttype and rustybuzz.

use super::text_shaping::FontBytes;
use crate::renderer::text::FontFeature;
use ab_glyph_rasterizer::{Point as AbPoint, Rasterizer};
use harfrust::{
    Buffer, ClusterLevel, Direction, Feature, Language, Script, ShapeOptions, ShapePlan,
    ShaperFont, Tag,
};
use skrifa::font::glyph::GlyphSourceOrder;
use skrifa::font::glyph::outline::{DrawMemory, DrawOptions, PathSink};
use skrifa::{Font, GlyphId};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

const MAX_CACHED_PLANS: usize = 32;

type CachedPlan = (
    Direction,
    Option<Script>,
    Option<Language>,
    Vec<FontFeature>,
    Arc<ShapePlan>,
);

/// The one parsed font that shaping, boxes and outlines read: `skrifa::Font`
/// is `read_fonts::model::Font`, the type `harfrust::ShaperFont` and
/// `ShapePlan` take, so this compiles only while both share it.
#[derive(Clone)]
pub(crate) struct FontationsFace {
    pub(crate) font: Font,
    plans: Arc<Mutex<VecDeque<CachedPlan>>>,
}

impl FontationsFace {
    pub(crate) fn new(data: &Arc<FontBytes>, index: u32) -> Option<Self> {
        let blob: Arc<dyn AsRef<[u8]> + Send + Sync> = data.clone();
        let font = Font::new(skrifa::font::Blob::from(blob), index)?;
        Some(Self {
            font,
            plans: Arc::default(),
        })
    }

    fn plan_for(&self, buffer: &Buffer, features: &[FontFeature]) -> Arc<ShapePlan> {
        let (direction, script, language) = (
            buffer.direction(),
            buffer.script(),
            buffer.language().cloned(),
        );
        let mut plans = self.plans.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = plans.iter().rposition(|(d, s, l, f, _)| {
            *d == direction && *s == script && *l == language && f == features
        }) {
            let entry = plans.remove(index).expect("matching plan");
            let plan = Arc::clone(&entry.4);
            plans.push_back(entry);
            return plan;
        }
        let plan = Arc::new(ShapePlan::new(
            &self.font,
            direction,
            script,
            language.as_ref(),
            &harfrust_features(features),
        ));
        if plans.len() == MAX_CACHED_PLANS {
            plans.pop_front();
        }
        plans.push_back((
            direction,
            script,
            language,
            features.to_vec(),
            Arc::clone(&plan),
        ));
        plan
    }

    /// Shapes `chars` (codepoint, cluster) into
    /// `(glyph, cluster, x_advance, x_offset, y_offset)` in font units.
    pub(crate) fn shape(
        &self,
        chars: impl Iterator<Item = (char, u32)>,
        script_tag: Option<[u8; 4]>,
        rtl: bool,
        features: &[FontFeature],
    ) -> Vec<(u32, u32, i32, i32, i32)> {
        let mut buffer = Buffer::new();
        buffer.set_cluster_level(ClusterLevel::MonotoneGraphemes);
        buffer.set_direction(if rtl {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        });
        if let Some(tag) = script_tag {
            buffer.set_script(Script::from_iso15924_tag(Tag::new(&tag)));
        }
        for (ch, cluster) in chars {
            buffer.push(ch as u32, cluster);
        }
        buffer.guess_segment_properties();
        let plan = self.plan_for(&buffer, features);
        let shaper = ShaperFont::new(&self.font);
        let features = harfrust_features(features);
        // A plan for the buffer's own properties cannot fail.
        let _ = harfrust::shape(
            &shaper,
            &mut buffer,
            ShapeOptions::new().plan(Some(&plan)).features(&features),
        );
        buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions())
            .map(|(info, pos)| {
                (
                    info.glyph_id,
                    info.cluster,
                    pos.x_advance,
                    pos.x_offset,
                    pos.y_offset,
                )
            })
            .collect()
    }

    /// The outline's box in font units: `x_min, y_min, x_max, y_max`. For
    /// `glyf` it is the glyph header's box, with `x_min` from `hmtx` as
    /// HarfBuzz states it, and the outline is not decoded.
    pub(crate) fn font_units_box(&self, id: u16) -> Option<[i16; 4]> {
        let extents = self
            .font
            .glyph_metrics()
            .with_extents_source_order(GlyphSourceOrder::BASE_OUTLINES)
            .extents(GlyphId::new(u32::from(id)))?;
        if extents.width == 0.0 || extents.height == 0.0 {
            return None;
        }
        let x_min = extents.x_bearing;
        let y_max = extents.y_bearing;
        Some([
            x_min.floor() as i16,
            (y_max - extents.height.abs()).floor() as i16,
            (x_min + extents.width).ceil() as i16,
            y_max.ceil() as i16,
        ])
    }

    /// Coverage of glyph `id` scaled by `scale` (x, y) per font unit, its origin at
    /// `offset` pixels from the top left of a `width` x `height` box, as
    /// rusttype's `PositionedGlyph::draw` gives it.
    pub(crate) fn draw(
        &self,
        id: u16,
        scale: (f32, f32),
        offset: (f32, f32),
        (width, height): (u32, u32),
        mut o: impl FnMut(u32, u32, f32),
    ) {
        let Some(glyph) = self.font.outline_glyphs().get(GlyphId::new(u32::from(id))) else {
            return;
        };
        let mut sink = RasterSink {
            rasterizer: Rasterizer::new(width as usize, height as usize),
            scale,
            offset,
            last: AbPoint { x: 0.0, y: 0.0 },
            last_move: None,
        };
        if glyph
            .draw(&DrawOptions::new(), DrawMemory::Scratch, &mut sink)
            .is_err()
        {
            return;
        }
        sink.rasterizer.for_each_pixel_2d(&mut o);
    }
}

fn harfrust_features(features: &[FontFeature]) -> Vec<Feature> {
    features
        .iter()
        .map(|f| Feature::new(Tag::new(&f.tag), f.value, ..))
        .collect()
}

/// rusttype's scale (y flipped), then its translation, then ab_glyph_rasterizer.
struct RasterSink {
    rasterizer: Rasterizer,
    scale: (f32, f32),
    offset: (f32, f32),
    last: AbPoint,
    last_move: Option<AbPoint>,
}

impl RasterSink {
    fn point(&self, x: f32, y: f32) -> AbPoint {
        AbPoint {
            x: x * self.scale.0 + self.offset.0,
            y: y * -self.scale.1 + self.offset.1,
        }
    }
}

impl PathSink for RasterSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.last = self.point(x, y);
        self.last_move = Some(self.last);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.rasterizer.draw_line(self.last, p);
        self.last = p;
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.point(cx0, cy0), self.point(x, y));
        self.rasterizer.draw_quad(self.last, c, p);
        self.last = p;
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c0, c1, p) = (self.point(cx0, cy0), self.point(cx1, cy1), self.point(x, y));
        self.rasterizer.draw_cubic(self.last, c0, c1, p);
        self.last = p;
    }

    fn close(&mut self) {
        if let Some(m) = self.last_move {
            self.rasterizer.draw_line(self.last, m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::system_fonts::{font_candidates, read_font_file};

    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
    }

    /// `(resident, phys_footprint)` in MB and retired instructions in millions,
    /// from `rusage_info_v4`.
    fn memory_mb() -> (f64, f64, f64) {
        let mut words = [0u64; 64];
        // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
        let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
        assert_eq!(status, 0);
        (
            words[8] as f64 / 1_048_576.0,
            words[9] as f64 / 1_048_576.0,
            words[31] as f64 / 1e6,
        )
    }

    /// Memory after parsing every face of the system font set, one engine per
    /// process: `RUI_369_MODE=current` (rusttype font + rustybuzz face) or
    /// `candidate` (one read-fonts `Font` for HarfRust and outlines).
    /// Each face then shapes a short string so its layout tables are read.
    #[test]
    #[ignore = "measures this process; run one mode per process"]
    fn rss_after_fallback_load() {
        let mode = std::env::var("RUI_369_MODE").unwrap_or_default();
        let files: Vec<Arc<FontBytes>> = font_candidates()
            .filter_map(|(_, path)| read_font_file(&path).ok().map(Arc::new))
            .collect();
        let before = memory_mb();
        let started = std::time::Instant::now();
        let mut current = Vec::new();
        let mut candidate = Vec::new();
        let mut faces = 0;
        for data in &files {
            for index in 0..64 {
                match mode.as_str() {
                    "current" => {
                        // SAFETY: `files` outlives `current`, which is dropped first.
                        let bytes: &'static [u8] = unsafe { std::mem::transmute(data.as_slice()) };
                        let (Some(font), Some(face)) = (
                            rusttype::Font::try_from_bytes_and_index(bytes, index),
                            crate::renderer::text_default_instance::default_instance_face(
                                bytes, index,
                            ),
                        ) else {
                            break;
                        };
                        let mut buffer = rustybuzz::UnicodeBuffer::new();
                        buffer.push_str("Ag 文");
                        buffer.guess_segment_properties();
                        std::hint::black_box(rustybuzz::shape(&face, &[], buffer));
                        current.push((font, face));
                    }
                    "candidate" | "candidate_parse_only" => {
                        let Some(face) = FontationsFace::new(data, index) else {
                            break;
                        };
                        if mode == "candidate_parse_only" {
                            candidate.push(face);
                            faces += 1;
                            if !data.as_slice().starts_with(b"ttcf") {
                                break;
                            }
                            continue;
                        }
                        let shaped = face.shape("Ag 文".chars().map(|c| (c, 0)), None, false, &[]);
                        std::hint::black_box(shaped);
                        candidate.push(face);
                    }
                    other => panic!("RUI_369_MODE={other:?}"),
                }
                faces += 1;
                if !data.as_slice().starts_with(b"ttcf") {
                    break;
                }
            }
        }
        let elapsed_ms = started.elapsed().as_secs_f64() * 1e3;
        let after = memory_mb();
        println!(
            "rss369 mode={mode} files={} faces={faces} load_instructions_m={:.2} load_ms={elapsed_ms:.2} resident_mb={:.1}->{:.1} (+{:.1}) footprint_mb={:.1}->{:.1} (+{:.1})",
            files.len(),
            after.2 - before.2,
            before.0,
            after.0,
            after.0 - before.0,
            before.1,
            after.1,
            after.1 - before.1,
        );
        drop(current);
        drop(candidate);
    }
}
