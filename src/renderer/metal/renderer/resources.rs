//! Textures the renderer keeps: uploaded images and the offscreen target.

use super::*;
use crate::renderer::plan::TextureUpload;

impl MetalRenderer {
    /// The kept offscreen texture, at least `width` by `height`. It only grows,
    /// in steps of [`OFFSCREEN_GROWTH`] pixels, and frames draw into its top
    /// left, so resizing back and forth reuses one texture.
    pub(super) fn offscreen_texture(&mut self, width: u64, height: u64) -> Texture {
        match &self.offscreen_target {
            Some(texture) if texture.width() >= width && texture.height() >= height => {
                texture.clone()
            }
            kept => {
                let (kept_width, kept_height) = kept
                    .as_ref()
                    .map_or((0, 0), |texture| (texture.width(), texture.height()));
                let texture = create_capture_texture(
                    &self.device,
                    width.max(kept_width).next_multiple_of(OFFSCREEN_GROWTH),
                    height.max(kept_height).next_multiple_of(OFFSCREEN_GROWTH),
                );
                self.offscreen_target = Some(texture.clone());
                texture
            }
        }
    }
}

impl TextureUpload for Device {
    type Texture = Texture;

    fn upload(&self, size: Size, pixels: &[u8]) -> Texture {
        let width = size.width.max(1.0).round() as u64;
        let height = size.height.max(1.0).round() as u64;

        let desc = TextureDescriptor::new();
        desc.set_texture_type(MTLTextureType::D2);
        desc.set_pixel_format(MTLPixelFormat::RGBA8Unorm_sRGB);
        desc.set_width(width);
        desc.set_height(height);
        desc.set_usage(MTLTextureUsage::ShaderRead);

        let texture = self.new_texture(&desc);

        let bytes_per_row = width * 4;
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        };

        texture.replace_region(region, 0, pixels.as_ptr() as *const _, bytes_per_row);
        texture
    }
}
