//! Reading a frame back from the GPU and saving it as a PNG, to check what a fixture
//! shows. Only with `EGUI_COMPARE_PNG=<dir>`; never inside a measured frame.

use egui_wgpu::wgpu;

/// Copies `texture` (BGRA, `width` x `height`) to `<dir>/egui_<case>.png` when
/// `EGUI_COMPARE_PNG` names a directory.
pub fn save_png_if_asked(
    case: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    (width, height): (u32, u32),
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(dir) = std::env::var_os("EGUI_COMPARE_PNG") else {
        return Ok(());
    };
    // 2880 BGRA pixels a row is a multiple of wgpu's 256-byte row alignment.
    let row_bytes = width * 4;
    assert_eq!(row_bytes % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("egui_compare readback"),
        size: u64::from(row_bytes * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |result| {
        result.expect("readback buffer maps");
    });
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let mut pixels = buffer.slice(..).get_mapped_range()?.to_vec();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let path = std::path::Path::new(&dir).join(format!("egui_{case}.png"));
    let mut encoder = png::Encoder::new(std::fs::File::create(&path)?, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    eprintln!("saved {}", path.display());
    Ok(())
}
