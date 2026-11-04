//! Glyph sprites and the atlas page textures they sample.

use crate::renderer::text::TextGlyphCache;
use metal::{
    DeviceRef, MTLOrigin, MTLPixelFormat, MTLRegion, MTLSize, MTLTextureType, MTLTextureUsage,
    Texture, TextureDescriptor,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct GpuGlyph {
    pub bounds: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

/// One R8 texture per atlas page, kept in step with the text cache's atlas.
#[derive(Default)]
pub(super) struct GlyphPages {
    epoch: u64,
    textures: Vec<Texture>,
}

impl GlyphPages {
    /// Creates textures for new pages and copies newly rasterized glyphs into them.
    /// Uploads only write atlas space no earlier draw samples, so frames still in
    /// flight are unaffected.
    pub(super) fn sync(&mut self, device: &DeviceRef, cache: &mut TextGlyphCache) {
        if cache.epoch() != self.epoch {
            self.textures.clear();
            self.epoch = cache.epoch();
        }
        let atlas = cache.atlas();
        for page in self.textures.len()..atlas.page_count() {
            let Some((width, height)) = atlas.page_extent(page) else {
                break;
            };
            let desc = TextureDescriptor::new();
            desc.set_texture_type(MTLTextureType::D2);
            desc.set_pixel_format(MTLPixelFormat::R8Unorm);
            desc.set_width(u64::from(width));
            desc.set_height(u64::from(height));
            desc.set_usage(MTLTextureUsage::ShaderRead);
            self.textures.push(device.new_texture(&desc));
        }

        for upload in cache.take_uploads() {
            let rect = upload.slot.rect;
            let region = MTLRegion {
                origin: MTLOrigin {
                    x: u64::from(rect.x),
                    y: u64::from(rect.y),
                    z: 0,
                },
                size: MTLSize {
                    width: u64::from(rect.width),
                    height: u64::from(rect.height),
                    depth: 1,
                },
            };
            self.textures[upload.slot.page].replace_region(
                region,
                0,
                upload.pixels.as_ptr() as *const _,
                u64::from(rect.width),
            );
        }
    }

    pub(super) fn page(&self, page: usize) -> Option<&Texture> {
        self.textures.get(page)
    }
}
