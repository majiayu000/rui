//! Glyph sprites and the atlas page textures they sample.

use super::batch::InstancePool;
use crate::renderer::text::{GlyphAtlas, GlyphUpload};
use metal::{
    Buffer, CommandBufferRef, DeviceRef, MTLBlitOption, MTLOrigin, MTLPixelFormat, MTLSize,
    MTLStorageMode, MTLTextureType, MTLTextureUsage, Texture, TextureDescriptor,
};

/// Rows in the staging buffer start on this many bytes.
const ROW_ALIGNMENT: usize = 4;

/// One texture per atlas page, kept in step with one of the text cache's
/// atlases: R8 coverage, or sRGB RGBA color glyphs.
pub(super) struct GlyphPages {
    format: MTLPixelFormat,
    bytes_per_pixel: usize,
    epoch: u64,
    textures: Vec<Texture>,
}

impl GlyphPages {
    pub(super) fn coverage() -> Self {
        Self::new(MTLPixelFormat::R8Unorm, 1)
    }

    pub(super) fn color() -> Self {
        Self::new(MTLPixelFormat::RGBA8Unorm_sRGB, 4)
    }

    fn new(format: MTLPixelFormat, bytes_per_pixel: usize) -> Self {
        Self {
            format,
            bytes_per_pixel,
            epoch: 0,
            textures: Vec::new(),
        }
    }

    /// Creates textures for new pages, clears new and evicted pages, and copies
    /// newly rasterized glyphs in, all through a staging buffer and a blit in
    /// `command_buffer`. The blit runs on the GPU before this frame's drawing and
    /// after earlier frames' (the page textures are hazard tracked), so a page a
    /// frame in flight still samples is never written under it. Returns the
    /// staging buffer, to retire with the command buffer, if there was anything
    /// to copy.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn sync(
        &mut self,
        device: &DeviceRef,
        epoch: u64,
        atlas: &GlyphAtlas,
        mut cleared: Vec<usize>,
        uploads: Vec<GlyphUpload>,
        command_buffer: &CommandBufferRef,
        staging: &mut InstancePool,
    ) -> Option<Buffer> {
        if epoch != self.epoch {
            self.textures.clear();
            self.epoch = epoch;
        }
        for page in self.textures.len()..atlas.page_count() {
            let Some((width, height)) = atlas.page_extent(page) else {
                break;
            };
            let desc = TextureDescriptor::new();
            desc.set_texture_type(MTLTextureType::D2);
            desc.set_pixel_format(self.format);
            desc.set_width(u64::from(width));
            desc.set_height(u64::from(height));
            desc.set_storage_mode(MTLStorageMode::Private);
            desc.set_usage(MTLTextureUsage::ShaderRead);
            self.textures.push(device.new_texture(&desc));
            // A private texture starts undefined. Sprites sample their texels at
            // the centers, but rounding in the filter may weigh in a neighbour,
            // and the padding between glyphs is meant to read 0.
            cleared.push(page);
        }
        cleared.sort_unstable();
        cleared.dedup();
        if cleared.is_empty() && uploads.is_empty() {
            return None;
        }

        // Staging layout: one zeroed page for every clear to copy from, then each
        // upload's rows.
        let bytes_per_pixel = self.bytes_per_pixel;
        let row_bytes =
            |width: u32| (width as usize * bytes_per_pixel).next_multiple_of(ROW_ALIGNMENT);
        let zero_bytes = cleared
            .iter()
            .filter_map(|&page| atlas.page_extent(page))
            .map(|(width, height)| row_bytes(width) * height as usize)
            .max()
            .unwrap_or(0);
        let upload_bytes: usize = uploads
            .iter()
            .map(|upload| row_bytes(upload.slot.rect.width) * upload.slot.rect.height as usize)
            .sum();
        let buffer = staging.acquire(device, zero_bytes + upload_bytes);
        let contents = buffer.contents() as *mut u8;
        // SAFETY: `acquire` returned a buffer of at least `zero_bytes +
        // upload_bytes`, no command buffer the GPU has not finished reads it,
        // and every write below stays within those bytes.
        unsafe {
            std::ptr::write_bytes(contents, 0, zero_bytes);
            let mut offset = zero_bytes;
            for upload in &uploads {
                let width = upload.slot.rect.width as usize * bytes_per_pixel;
                for row in upload.pixels.chunks_exact(width) {
                    std::ptr::copy_nonoverlapping(row.as_ptr(), contents.add(offset), width);
                    offset += row_bytes(upload.slot.rect.width);
                }
            }
        }

        let blit = command_buffer.new_blit_command_encoder();
        for &page in &cleared {
            let Some((width, height)) = atlas.page_extent(page) else {
                continue;
            };
            blit.copy_from_buffer_to_texture(
                &buffer,
                0,
                row_bytes(width) as u64,
                0,
                MTLSize::new(u64::from(width), u64::from(height), 1),
                &self.textures[page],
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLBlitOption::empty(),
            );
        }
        let mut offset = zero_bytes;
        for upload in &uploads {
            let rect = upload.slot.rect;
            blit.copy_from_buffer_to_texture(
                &buffer,
                offset as u64,
                row_bytes(rect.width) as u64,
                0,
                MTLSize::new(u64::from(rect.width), u64::from(rect.height), 1),
                &self.textures[upload.slot.page],
                0,
                0,
                MTLOrigin {
                    x: u64::from(rect.x),
                    y: u64::from(rect.y),
                    z: 0,
                },
                MTLBlitOption::empty(),
            );
            offset += row_bytes(rect.width) * rect.height as usize;
        }
        blit.end_encoding();
        Some(buffer)
    }

    pub(super) fn page(&self, page: usize) -> Option<&Texture> {
        self.textures.get(page)
    }
}
