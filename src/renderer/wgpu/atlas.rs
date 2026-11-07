//! The atlas page textures glyph and path sprites sample, kept in step with
//! the text cache's atlases.

use crate::renderer::text::{GlyphAtlas, GlyphUpload};

/// One texture per atlas page: R8 coverage, or sRGB RGBA color glyphs.
pub(super) struct GlyphPages {
    format: wgpu::TextureFormat,
    bytes_per_pixel: u32,
    epoch: u64,
    pages: Vec<wgpu::TextureView>,
    /// Bumped whenever a page's texture is replaced, so bind groups naming
    /// pages can tell they are stale.
    generation: u64,
}

impl GlyphPages {
    pub(super) fn coverage() -> Self {
        Self::new(wgpu::TextureFormat::R8Unorm, 1)
    }

    pub(super) fn color() -> Self {
        Self::new(wgpu::TextureFormat::Rgba8UnormSrgb, 4)
    }

    fn new(format: wgpu::TextureFormat, bytes_per_pixel: u32) -> Self {
        Self {
            format,
            bytes_per_pixel,
            epoch: 0,
            pages: Vec::new(),
            generation: 0,
        }
    }

    /// Creates textures for new pages, clears evicted pages, and copies newly
    /// rasterized glyphs in. Queue writes run before the next submission's
    /// commands and after earlier submissions', so a page a frame in flight
    /// still samples is never written under it.
    pub(super) fn sync(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        epoch: u64,
        atlas: &GlyphAtlas,
        cleared: Vec<usize>,
        uploads: Vec<GlyphUpload>,
    ) {
        if epoch != self.epoch {
            self.pages.clear();
            self.epoch = epoch;
            self.generation += 1;
        }
        let created = self.pages.len();
        for page in created..atlas.page_count() {
            let Some((width, height)) = atlas.page_extent(page) else {
                break;
            };
            // wgpu zeroes new textures, so the padding between glyphs reads 0.
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("rui glyph page"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.pages.push(texture.create_view(&Default::default()));
        }
        for page in cleared {
            if page >= created {
                continue;
            }
            let (Some(view), Some((width, height))) =
                (self.pages.get(page), atlas.page_extent(page))
            else {
                continue;
            };
            let zeros = vec![0; (width * height * self.bytes_per_pixel) as usize];
            self.write(queue, view.texture(), [0, 0, width, height], &zeros);
        }
        for upload in uploads {
            let rect = upload.slot.rect;
            if let Some(view) = self.pages.get(upload.slot.page) {
                self.write(
                    queue,
                    view.texture(),
                    [rect.x, rect.y, rect.width, rect.height],
                    &upload.pixels,
                );
            }
        }
    }

    /// Writes tightly packed rows into `[x, y, width, height]` of `texture`.
    fn write(&self, queue: &wgpu::Queue, texture: &wgpu::Texture, rect: [u32; 4], pixels: &[u8]) {
        let [x, y, width, height] = rect;
        if width == 0 || height == 0 {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * self.bytes_per_pixel),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub(super) fn page(&self, page: usize) -> Option<&wgpu::TextureView> {
        self.pages.get(page)
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
}
