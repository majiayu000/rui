use crate::renderer::RendererError;
use metal::{
    Device, MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTextureType, MTLTextureUsage,
    Texture, TextureDescriptor, TextureRef,
};
use std::ffi::c_void;

pub(super) fn texture_extent(value: f32, axis: &str) -> Result<u64, RendererError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(RendererError::render_failed(format!(
            "offscreen capture {axis} must be positive and finite"
        )));
    }
    Ok(value.round().max(1.0) as u64)
}

pub(super) fn create_capture_texture(device: &Device, width: u64, height: u64) -> Texture {
    let desc = TextureDescriptor::new();
    desc.set_texture_type(MTLTextureType::D2);
    desc.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    desc.set_width(width);
    desc.set_height(height);
    desc.set_storage_mode(MTLStorageMode::Shared);
    desc.set_usage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
    device.new_texture(&desc)
}

pub(super) fn read_texture_rgba(texture: &TextureRef, width: u64, height: u64) -> Vec<u8> {
    let bytes_per_row = width * 4;
    let mut pixels = vec![0; (bytes_per_row * height) as usize];
    texture.get_bytes(
        pixels.as_mut_ptr() as *mut c_void,
        bytes_per_row,
        MTLRegion {
            origin: metal::MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        },
        0,
    );

    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    pixels
}
