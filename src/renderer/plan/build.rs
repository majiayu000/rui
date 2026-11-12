//! Planning a frame: the scene becomes instance data and a plan of draws.

use super::frame::{ItemKind, Pages};
use super::order::Rect;
use super::path::PathGeometry;
use super::retained::{damage_scissor, intersect_scissors};
use super::sprites::SpriteLookup;
use super::*;
use crate::core::geometry::Bounds;
use crate::elements::fit_bounds;
use crate::renderer::Scene;
use crate::renderer::primitives::Primitive;
use crate::renderer::text::TextRequest;
use crate::{ImageFit, ImageSource};

impl<T> FrameData<T> {
    pub(super) fn clear(&mut self) {
        self.plan.clear();
        self.quads.clear();
        self.shadows.clear();
        self.glyphs.clear();
        self.images.clear();
        self.path_tiles.clear();
        self.path_edges.clear();
        self.scissors.clear();
        self.clip_edges.clear();
        self.clip_bounds.clear();
    }

    /// Adds a clip rectangle and returns its index.
    pub(super) fn push_clip(&mut self, scissor: ScissorRect, scale_factor: f32) -> u32 {
        let scissor = match self.damage {
            Some(damage) => intersect_scissors(scissor, damage),
            None => scissor,
        };
        self.scissors.push(scissor);
        let edges = [
            scissor.x as f32,
            scissor.y as f32,
            (scissor.x + scissor.width) as f32,
            (scissor.y + scissor.height) as f32,
        ];
        let frame = self.clip_edges.first().copied().unwrap_or(edges);
        self.clip_bounds
            .push(simple::clip_bounds(edges, frame, scale_factor));
        self.clip_edges.push(edges);
        (self.scissors.len() - 1) as u32
    }

    // Called for every primitive drawn: left to the inliner, whether it is
    // inlined into `push_quad` changed with unrelated code elsewhere in the crate.
    #[inline]
    pub(super) fn push_item(&mut self, kind: ItemKind, clip: u32, rect: Rect, scale_factor: f32) {
        if let Some(rect) = visible_rect(&self.clip_edges, rect, clip, scale_factor) {
            self.plan.push(kind, clip, Some(rect));
        }
    }

    pub(super) fn push_quad(&mut self, mut quad: GpuQuad, clip: u32, scale_factor: f32) {
        let rect = bounds_rect(quad.bounds);
        let start = self.quads.len() as u32;
        // Without branches: most quads are plain and aligned, or most are not.
        let aligned = quad.is_plain() & simple::on_device_pixels(rect, scale_factor);
        quad.gradient_params[2] = f32::from(u8::from(aligned));
        self.quads.push(quad);
        let Some(visible) = visible_rect(&self.clip_edges, rect, clip, scale_factor) else {
            return;
        };
        // Most clips cut nothing the scissor does not; in the others a quad
        // the clip leaves whole needs no clip test.
        let inside = self.clip_bounds[clip as usize].is_none() || visible == rect;
        let kind = ItemKind::Sprites {
            kind: KIND_QUAD,
            start,
            count: 1,
            pages: Pages::default(),
            simple: aligned & inside,
        };
        self.plan.push(kind, clip, Some(visible));
    }
}

/// `rect` cut to clip `clip`, in logical pixels; `None` when nothing is left.
pub(super) fn visible_rect(
    clip_edges: &[[f32; 4]],
    rect: Rect,
    clip: u32,
    scale_factor: f32,
) -> Option<Rect> {
    let edges = clip_edges[clip as usize];
    let visible = [
        rect[0].max(edges[0] / scale_factor),
        rect[1].max(edges[1] / scale_factor),
        rect[2].min(edges[2] / scale_factor),
        rect[3].min(edges[3] / scale_factor),
    ];
    (visible[0] < visible[2] && visible[1] < visible[3]).then_some(visible)
}

/// `[x, y, width, height]` as the rectangle it covers.
pub(super) fn bounds_rect(bounds: [f32; 4]) -> Rect {
    let [x, y, width, height] = bounds;
    [
        x.min(x + width),
        y.min(y + height),
        x.max(x + width),
        y.max(y + height),
    ]
}

pub(super) fn outset(rect: Rect, by: f32) -> Rect {
    [rect[0] - by, rect[1] - by, rect[2] + by, rect[3] + by]
}

pub(super) fn union(a: Rect, b: Rect) -> Rect {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// The device pixels `clip` keeps, or the whole drawable without one.
fn scissor_rect(clip: Option<Bounds>, scale_factor: f32, drawable_size: Size) -> ScissorRect {
    let Some(bounds) = clip else {
        return ScissorRect {
            x: 0,
            y: 0,
            width: drawable_size.width as u64,
            height: drawable_size.height as u64,
        };
    };
    if bounds.is_empty() {
        return ScissorRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }

    let x = (bounds.x() * scale_factor).round().max(0.0) as u64;
    let y = (bounds.y() * scale_factor).round().max(0.0) as u64;
    let mut width = (bounds.width() * scale_factor).round().max(0.0) as u64;
    let mut height = (bounds.height() * scale_factor).round().max(0.0) as u64;

    let max_w = drawable_size.width as u64;
    let max_h = drawable_size.height as u64;

    if x + width > max_w {
        width = max_w.saturating_sub(x);
    }
    if y + height > max_h {
        height = max_h.saturating_sub(y);
    }

    ScissorRect {
        x,
        y,
        width,
        height,
    }
}

impl<T: Clone> FramePlanner<T> {
    /// Turns the scene into instance data and a plan of draws. With `damage`,
    /// only what can touch it, cut to it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_frame(
        &mut self,
        frame: &mut FrameData<T>,
        scene: &Scene,
        viewport_size: Size,
        scale_factor: f32,
        drawable_size: Size,
        damage: Option<Bounds>,
        upload: &impl TextureUpload<Texture = T>,
    ) -> Result<(), RendererError> {
        frame.clear();
        frame.damage = damage.map(|damage| damage_scissor(damage, scale_factor, drawable_size));
        // The damage as the device pixels it covers, which is what gets drawn.
        let redrawn = frame.damage.map(|scissor| {
            Bounds::from_xywh(
                scissor.x as f32 / scale_factor,
                scissor.y as f32 / scale_factor,
                scissor.width as f32 / scale_factor,
                scissor.height as f32 / scale_factor,
            )
        });
        let viewport = Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height);
        let mut clip_stack: Vec<Bounds> = Vec::new();
        let mut clip = frame.push_clip(
            scissor_rect(None, scale_factor, drawable_size),
            scale_factor,
        );

        for primitive in scene.primitives() {
            // Nothing a primitive outside the current clip draws can be seen,
            // nor, in a retained frame, anything outside the damage.
            let visible = clip_stack.last().copied().unwrap_or(viewport);
            let visible = match redrawn {
                Some(redrawn) => visible.intersection(&redrawn),
                None => Some(visible),
            };
            let clips = matches!(primitive, Primitive::PushClip { .. } | Primitive::PopClip);
            if !clips && !visible.is_some_and(|visible| primitive.may_touch(&visible)) {
                continue;
            }
            match primitive {
                Primitive::Shadow {
                    bounds,
                    corner_radii,
                    blur_radius,
                    color,
                } => {
                    let shadow =
                        GpuShadow::from_primitive(*bounds, *corner_radii, *blur_radius, *color);
                    // As far as the shader's blur margin reaches, and half a
                    // pixel more for rounding.
                    let margin = 3.0 * (blur_radius.max(0.0) * 0.5) + 1.5;
                    let kind = ItemKind::Sprites {
                        kind: KIND_SHADOW,
                        start: frame.shadows.len() as u32,
                        count: 1,
                        pages: Pages::default(),
                        simple: false,
                    };
                    frame.shadows.push(shadow);
                    let rect = outset(bounds_rect(shadow.bounds), margin);
                    frame.push_item(kind, clip, rect, scale_factor);
                }
                Primitive::Quad {
                    bounds,
                    background,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let quad = GpuQuad::solid(
                        *bounds,
                        *background,
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    frame.push_quad(quad, clip, scale_factor);
                }
                Primitive::LinearGradient {
                    bounds,
                    start,
                    end,
                    angle,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let quad = GpuQuad::linear_gradient(
                        *bounds,
                        *start,
                        *end,
                        angle.to_radians(),
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    frame.push_quad(quad, clip, scale_factor);
                }
                Primitive::RadialGradient {
                    bounds,
                    inner,
                    outer,
                    border_color,
                    border_widths,
                    corner_radii,
                } => {
                    let quad = GpuQuad::radial_gradient(
                        *bounds,
                        *inner,
                        *outer,
                        *border_color,
                        *border_widths,
                        *corner_radii,
                    );
                    frame.push_quad(quad, clip, scale_factor);
                }
                Primitive::Image {
                    bounds,
                    source,
                    fit,
                    corner_radii,
                    opacity,
                } => {
                    self.build_image(
                        frame,
                        bounds,
                        source,
                        *fit,
                        *corner_radii,
                        *opacity,
                        clip,
                        clip_stack.last().copied(),
                        scale_factor,
                        drawable_size,
                        upload,
                    )?;
                }
                Primitive::Text {
                    bounds,
                    content,
                    color,
                    font_size,
                    font_weight,
                    font_family,
                    line_height,
                    align,
                } => {
                    self.build_text(
                        frame,
                        bounds,
                        content,
                        *color,
                        *font_size,
                        *font_weight,
                        font_family.as_deref(),
                        *line_height,
                        *align,
                        clip,
                        scale_factor,
                    )?;
                }
                Primitive::Path {
                    vertices,
                    color,
                    stroke_width,
                } => {
                    let mut path = std::mem::take(&mut frame.path);
                    let built = path
                        .set(vertices, *stroke_width, scale_factor)
                        .and_then(|()| self.build_path(frame, &path, *color, clip, scale_factor));
                    frame.path = path;
                    built?;
                }
                Primitive::PushClip { bounds, .. } => {
                    let new_clip = if let Some(prev) = clip_stack.last() {
                        prev.intersection(bounds)
                            .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
                    } else {
                        *bounds
                    };
                    clip_stack.push(new_clip);
                    clip = frame.push_clip(
                        scissor_rect(clip_stack.last().copied(), scale_factor, drawable_size),
                        scale_factor,
                    );
                }
                Primitive::PopClip => {
                    clip_stack.pop();
                    clip = frame.push_clip(
                        scissor_rect(clip_stack.last().copied(), scale_factor, drawable_size),
                        scale_factor,
                    );
                }
            }
        }

        // Text runs are pushed without their extent, which only ordering needs.
        let FrameData {
            plan,
            glyphs,
            clip_edges,
            ..
        } = frame;
        plan.plan(
            viewport_size.width.max(1.0),
            viewport_size.height.max(1.0),
            |kind, clip| {
                let ItemKind::Sprites { start, count, .. } = kind else {
                    return None;
                };
                let rect = glyphs[start as usize..(start + count) as usize]
                    .iter()
                    .map(|glyph| bounds_rect(glyph.bounds))
                    .reduce(union)?;
                visible_rect(clip_edges, rect, clip, scale_factor)
            },
        );
        Ok(())
    }

    /// Draws `path` from the atlas when it is a small shape drawn before, and as
    /// analytic tiles otherwise.
    pub(super) fn build_path(
        &mut self,
        frame: &mut FrameData<T>,
        path: &PathGeometry,
        color: crate::core::color::Rgba,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        // A font loaded earlier in this frame may have emptied the atlas.
        self.path_sprites.sync_epoch(self.text_cache.epoch());
        let (cell, key) = match self.path_sprites.lookup(path) {
            SpriteLookup::Analytic => (None, 0),
            SpriteLookup::Placed(slot, generation, cell, key) => {
                if self.text_cache.use_sprite(slot, generation) {
                    (Some((Some(slot), cell)), key)
                } else {
                    // Its atlas page was emptied: place it again.
                    (Some((None, cell)), key)
                }
            }
            SpriteLookup::Rasterize(cell, key) => (Some((None, cell)), key),
        };
        let placed = match cell {
            None => None,
            Some((Some(slot), cell)) => Some((slot, cell)),
            Some((None, cell)) => {
                let pixels = path.rasterize(cell.origin, cell.width, cell.height);
                match self
                    .text_cache
                    .place_sprite(cell.width, cell.height, pixels)
                {
                    Ok((slot, generation)) => {
                        self.path_sprites.place(key, slot, generation);
                        Some((slot, cell))
                    }
                    // Without atlas room the path is still drawn, as tiles.
                    Err(RendererResourceError::ResourcePressure { .. }) => None,
                    Err(err) => return Err(err.into()),
                }
            }
        };

        if let Some((slot, cell)) = placed {
            let (page_width, page_height) = self
                .text_cache
                .atlas()
                .page_extent(slot.page)
                .ok_or_else(|| {
                    RendererResourceError::missing(RendererResourceKind::Glyph, slot.page as u32)
                })?;
            let rect = slot.rect;
            let sprite = GpuGlyph {
                bounds: [
                    cell.origin[0] / scale_factor,
                    cell.origin[1] / scale_factor,
                    cell.width as f32 / scale_factor,
                    cell.height as f32 / scale_factor,
                ],
                uv: [
                    rect.x as f32 / page_width as f32,
                    rect.y as f32 / page_height as f32,
                    rect.width as f32 / page_width as f32,
                    rect.height as f32 / page_height as f32,
                ],
                color: color.to_array(),
            };
            let kind = ItemKind::Sprites {
                kind: KIND_PATH_SPRITE,
                start: frame.glyphs.len() as u32,
                count: 1,
                pages: Pages::coverage(slot.page as u32),
                simple: false,
            };
            frame.glyphs.push(sprite);
            frame.push_item(kind, clip, bounds_rect(sprite.bounds), scale_factor);
            return Ok(());
        }

        let start = frame.path_tiles.len() as u32;
        let visible = frame.clip_edges[clip as usize];
        if let Some(rect) = self.path_tiles.push(
            path,
            color,
            scale_factor,
            visible,
            &mut frame.path_tiles,
            &mut frame.path_edges,
        ) {
            let kind = ItemKind::Sprites {
                kind: KIND_PATH,
                start,
                count: frame.path_tiles.len() as u32 - start,
                pages: Pages::default(),
                simple: false,
            };
            frame.push_item(kind, clip, rect, scale_factor);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_image(
        &mut self,
        frame: &mut FrameData<T>,
        bounds: &Bounds,
        source: &ImageSource,
        fit: ImageFit,
        corner_radii: crate::core::style::Corners,
        opacity: f32,
        clip: u32,
        current_clip: Option<Bounds>,
        scale_factor: f32,
        drawable_size: Size,
        upload: &impl TextureUpload<Texture = T>,
    ) -> Result<(), RendererError> {
        if let ImageSource::Texture(id) = source {
            let texture = self
                .textures
                .get(*id)
                .ok_or_else(|| RendererResourceError::missing(RendererResourceKind::Texture, *id))?
                .clone();
            let instance = GpuImage::new(*bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
            let rect = bounds_rect(instance.bounds);
            frame.images.push(ImageDraw {
                texture,
                instance,
                clip,
            });
            let kind = ItemKind::Image((frame.images.len() - 1) as u32);
            frame.push_item(kind, clip, rect, scale_factor);
            return Ok(());
        }

        let entry = self.image_cache.resolve(source)?;

        let texture =
            self.textures
                .ensure(entry.handle.id.as_u32(), entry.size, &entry.pixels, upload)?;

        let dest_bounds = fit_bounds(*bounds, entry.size, fit);

        // Clip to container for cover/contain
        let container_clip = if let Some(prev) = current_clip {
            prev.intersection(bounds)
                .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
        } else {
            *bounds
        };
        let image_clip = frame.push_clip(
            scissor_rect(Some(container_clip), scale_factor, drawable_size),
            scale_factor,
        );

        let instance = GpuImage::new(dest_bounds, corner_radii, [1.0, 1.0, 1.0, 1.0], opacity);
        let rect = bounds_rect(instance.bounds);
        frame.images.push(ImageDraw {
            texture,
            instance,
            clip: image_clip,
        });
        let kind = ItemKind::Image((frame.images.len() - 1) as u32);
        frame.push_item(kind, image_clip, rect, scale_factor);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_text(
        &mut self,
        frame: &mut FrameData<T>,
        bounds: &Bounds,
        content: &str,
        color: crate::core::color::Rgba,
        font_size: f32,
        font_weight: u16,
        font_family: Option<&str>,
        line_height: f32,
        align: crate::elements::text::TextAlign,
        clip: u32,
        scale_factor: f32,
    ) -> Result<(), RendererError> {
        let text_error =
            |err| RendererError::render_failed(format!("text rendering failed: {:?}", err));
        let shaped = match self.text_cache.shape(TextRequest::new(
            content,
            font_size,
            font_weight,
            font_family,
            line_height,
        )) {
            Ok(Some(shaped)) => shaped,
            Ok(None) => return Ok(()),
            Err(err) => return Err(text_error(err)),
        };

        let ink = shaped.metrics.ink_bounds;
        let mut x = bounds.x();
        match align {
            crate::elements::text::TextAlign::Left => {}
            crate::elements::text::TextAlign::Center => {
                x += (bounds.width() - ink.width()) * 0.5;
            }
            crate::elements::text::TextAlign::Right => {
                x += bounds.width() - ink.width();
            }
        }
        // Center the primary font's ascent and descent rather than the ink, so
        // every line of a wrapped paragraph lands on the same baseline.
        let font_height = shaped.ascent - shaped.descent;
        let line_top = bounds.y() + (bounds.height() - font_height) * 0.5;

        // Glyph x positions are relative to the ink box; baselines to the line top.
        let origin_x = x - ink.x();
        let origin_y = line_top;
        let device_font_size = font_size * scale_factor;
        let color = color.to_array();

        // Glyphs of one kind on one atlas page form one item; a color glyph, or
        // a glyph on another page, starts the next, ordered after it where they
        // overlap.
        // A run is simple when its glyphs are coverage glyphs inside the clip.
        let mut run: Option<(bool, u32, u32)> = None;
        // The flipped extent of the open run's glyphs.
        let unbounded = [f32::INFINITY; 4];
        let mut extent = unbounded;
        let bounds = frame.clip_bounds[clip as usize];
        let flush = |frame: &mut FrameData<T>, run: Option<(bool, u32, u32)>, extent: Rect| {
            if let Some((color, page, start)) = run {
                let (kind, pages) = if color {
                    (KIND_COLOR_GLYPH, Pages::color(page))
                } else {
                    (KIND_GLYPH, Pages::coverage(page))
                };
                let kind = ItemKind::Sprites {
                    kind,
                    start,
                    count: frame.glyphs.len() as u32 - start,
                    pages,
                    simple: !color && bounds.is_none_or(|bounds| simple::inside(&bounds, extent)),
                };
                frame.plan.push(kind, clip, None);
            }
        };

        let sprites = self
            .text_cache
            .sprites(&shaped, origin_x, device_font_size, scale_factor)
            .map_err(text_error)?;
        for sprite in sprites {
            let pen_y = ((origin_y + sprite.baseline_y) * scale_factor).round();
            let gpu = GpuGlyph {
                bounds: [
                    sprite.x,
                    (pen_y + sprite.top as f32) / scale_factor,
                    sprite.width,
                    sprite.height,
                ],
                uv: sprite.uv,
                color,
            };
            let key = (sprite.color, sprite.page);
            if run.is_none_or(|(color, open, _)| (color, open) != key) {
                flush(frame, run, extent);
                run = Some((key.0, key.1, frame.glyphs.len() as u32));
                extent = unbounded;
            }
            if bounds.is_some() {
                extent = simple::extend(extent, simple::flipped(bounds_rect(gpu.bounds)));
            }
            frame.glyphs.push(gpu);
        }
        flush(frame, run, extent);
        Ok(())
    }
}
