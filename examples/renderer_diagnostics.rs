use rui::renderer::{RecordingRenderer, Renderer};

fn main() {
    let renderer = RecordingRenderer::new();
    let diagnostics = renderer.diagnostics();

    println!("backend: {}", diagnostics.device.backend);
    println!("device: {}", diagnostics.device.device_name);
    println!("headless: {}", diagnostics.device.is_headless);
}
