//! Embeds the Metal shaders precompiled, so a first launch need not compile
//! them from source (about 200 ms for the sprite shader).
//!
//! With Xcode's `metal` tool each library is a `.metallib`. Without it (the
//! Command Line Tools alone do not have it) the build compiles the source on
//! this machine's Metal device and embeds the serialized `MTLBinaryArchive`,
//! which holds the AIR and so loads as a library on any Mac. With neither, or
//! if the embedded library does not load, the renderer compiles the source at
//! runtime and caches it (`src/renderer/metal/shader_cache.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/renderer/metal/shader_hash.rs"]
mod shader_hash;

const SHADERS: &str = "src/renderer/metal/shaders.rs";
const RETAINED: &str = "src/renderer/metal/retained.rs";

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rui_embedded_metallib)");
    println!("cargo::rerun-if-changed={SHADERS}");
    println!("cargo::rerun-if-changed={RETAINED}");
    println!("cargo::rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let metal_target = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
        && std::env::var_os("CARGO_FEATURE_METAL").is_some();
    if !metal_target {
        return;
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    // `xcrun -f metal` can succeed while the tool cannot run, as when Xcode's
    // Metal toolchain component is not installed.
    let tool = if has_metal_tool() {
        embed(&out, compile_with_tool)
    } else {
        Err(String::from("no metal tool"))
    };
    match tool.or_else(|_| embed(&out, compile_on_device)) {
        Ok(()) => println!("cargo::rustc-cfg=rui_embedded_metallib"),
        Err(err) => {
            println!("cargo::warning=shaders not precompiled, they compile at runtime: {err}")
        }
    }
}

fn has_metal_tool() -> bool {
    Command::new("xcrun")
        .args(["-sdk", "macosx", "-f", "metal"])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Compiles each shader library as the renderer assembles it, and writes
/// `embedded_shaders.rs` naming each with the hash of its source.
fn embed(out: &Path, compile: Compile) -> Result<(), String> {
    let read = |path: &str| std::fs::read_to_string(path).map_err(|err| format!("{path}: {err}"));
    let (shaders, retained) = (read(SHADERS)?, read(RETAINED)?);
    let color = raw_string(&shaders, "COLOR_SHADER")?;
    // Each library's pipelines, as the renderer creates them, as (vertex,
    // fragment, blended).
    let libraries: [(&str, String, &[Pipeline]); 3] = [
        (
            "sprite",
            format!("{color}{}", raw_string(&shaders, "SPRITE_SHADER")?),
            &[("sprite_vertex", "sprite_fragment", true)],
        ),
        (
            "image",
            format!("{color}{}", raw_string(&shaders, "IMAGE_SHADER")?),
            &[("image_vertex", "image_fragment", true)],
        ),
        (
            "copy",
            raw_string(&retained, "RETAINED_SHADER")?.to_string(),
            &[
                ("fullscreen_vertex", "copy_fragment", false),
                ("fullscreen_vertex", "clear_fragment", false),
            ],
        ),
    ];
    let mut arms = String::new();
    for (name, source, pipelines) in &libraries {
        let metallib = out.join(format!("{name}.metallib"));
        compile(out, name, source, pipelines, &metallib)?;
        let hash = shader_hash::shader_hash(&[source.as_bytes()]);
        arms.push_str(&format!(
            "        {name:?} => Some(({hash:#x}, include_bytes!({:?}))),\n",
            metallib.display().to_string()
        ));
    }
    let code = format!(
        "/// The embedded library named `name`: the hash of its source and its bytes.\n\
         pub(super) fn library(name: &str) -> Option<(u64, &'static [u8])> {{\n    \
         match name {{\n{arms}        _ => None,\n    }}\n}}\n"
    );
    std::fs::write(out.join("embedded_shaders.rs"), code).map_err(|err| err.to_string())
}

/// The contents of `const NAME: &str = r#"..."#;` in `file`.
fn raw_string<'a>(file: &'a str, name: &str) -> Result<&'a str, String> {
    let open = format!("const {name}: &str = r#\"");
    let start = file.find(&open).ok_or(format!("no {name}"))? + open.len();
    let end = file[start..]
        .find("\"#;")
        .ok_or(format!("{name} is not closed"))?
        + start;
    Ok(&file[start..end])
}

type Pipeline = (&'static str, &'static str, bool);
type Compile = fn(&Path, &str, &str, &[Pipeline], &Path) -> Result<(), String>;

fn compile_with_tool(
    out: &Path,
    name: &str,
    source: &str,
    _pipelines: &[Pipeline],
    metallib: &Path,
) -> Result<(), String> {
    let input = out.join(format!("{name}.metal"));
    std::fs::write(&input, source).map_err(|err| err.to_string())?;
    let mut command = Command::new("xcrun");
    command.args(["-sdk", "macosx", "metal"]);
    if let Ok(target) = std::env::var("MACOSX_DEPLOYMENT_TARGET") {
        command.arg(format!("-mmacosx-version-min={target}"));
    }
    let output = command
        .arg(&input)
        .arg("-o")
        .arg(metallib)
        .output()
        .map_err(|err| format!("running metal: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "metal failed on {name}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(target_os = "macos")]
fn compile_on_device(
    _out: &Path,
    _name: &str,
    source: &str,
    pipelines: &[Pipeline],
    metallib: &Path,
) -> Result<(), String> {
    use metal::*;
    let device = Device::system_default().ok_or("this machine has no Metal device")?;
    let library = device.new_library_with_source(source, &CompileOptions::new())?;
    let archive = device.new_binary_archive_with_descriptor(&BinaryArchiveDescriptor::new())?;
    for &(vertex, fragment, blended) in pipelines {
        let desc = RenderPipelineDescriptor::new();
        let vertex = library.get_function(vertex, None)?;
        let fragment = library.get_function(fragment, None)?;
        desc.set_vertex_function(Some(&vertex));
        desc.set_fragment_function(Some(&fragment));
        let color = desc
            .color_attachments()
            .object_at(0)
            .ok_or("no color attachment")?;
        // The renderer's target format and blending, so the GPU code matches.
        color.set_pixel_format(MTLPixelFormat::BGRA8Unorm_sRGB);
        color.set_blending_enabled(blended);
        if blended {
            color.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
            color.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
            color.set_source_alpha_blend_factor(MTLBlendFactor::One);
            color.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
        archive.add_render_pipeline_functions_with_descriptor(&desc)?;
    }
    let _ = std::fs::remove_file(metallib);
    let path = metallib.to_str().ok_or("OUT_DIR is not UTF-8")?;
    let mut url = String::from("file://");
    for &byte in path.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    // `URLWithString:` returns an autoreleased URL, which dropping a `URL`
    // would release a second time.
    let url = std::mem::ManuallyDrop::new(URL::new_with_string(&url));
    archive.serialize_to_url(&url)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn compile_on_device(
    _out: &Path,
    _name: &str,
    _source: &str,
    _pipelines: &[Pipeline],
    _metallib: &Path,
) -> Result<(), String> {
    Err("this build machine is not a Mac".to_string())
}
