//! Compiled shader libraries kept across launches.
//!
//! Compiling a shader from source takes about 200 ms on a cold start, before
//! the system's own shader cache holds it. A library is taken, in order, from:
//!
//! 1. a `.metallib` that `build.rs` compiled and embedded, when the build had
//!    Xcode's `metal` tool;
//! 2. an `MTLBinaryArchive` this process or an earlier one serialized under the
//!    per-user cache directory, keyed by the source, the device and the OS
//!    build. A serialized archive holds the AIR of its functions next to their
//!    GPU code, so it also loads as a library, without the source compile, and
//!    pipelines created with it attached skip the GPU compile too;
//! 3. the source, after which the archive is written for the next launch.
//!
//! The archive is a cache, never trusted for correctness: a missing,
//! unreadable or mismatched one is compiled again and rewritten.

use super::shader_hash::shader_hash;
use metal::*;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Overrides the cache directory; an empty value turns the cache off.
pub const RUI_SHADER_CACHE_DIR_ENV: &str = "RUI_SHADER_CACHE_DIR";

/// Bumped when what the cache stores changes, so older files are not read.
const CACHE_FORMAT: &[u8] = b"rui-shader-archive-1";

#[cfg(rui_embedded_metallib)]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_shaders.rs"));
}

/// Where a [`ShaderLibrary`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    #[cfg_attr(not(rui_embedded_metallib), allow(dead_code))]
    Embedded,
    Cached,
    Compiled,
}

impl Origin {
    pub(super) fn label(self) -> &'static str {
        match self {
            Origin::Embedded => "embedded",
            Origin::Cached => "cached",
            Origin::Compiled => "compiled",
        }
    }
}

/// A shader library and the archive its pipelines are looked up in.
pub(super) struct ShaderLibrary {
    pub(super) library: Library,
    pub(super) origin: Origin,
    /// How long getting the library took.
    pub(super) load_time: Duration,
    /// The archive this library was loaded from.
    archive: Option<BinaryArchive>,
    /// Where to write the archive, for a library compiled from source.
    store_at: Option<PathBuf>,
    name: String,
}

impl ShaderLibrary {
    /// The library named `name` compiled from `source`, which must define
    /// every function in `functions`.
    pub(super) fn load(
        device: &Device,
        name: &str,
        source: &str,
        functions: &[&str],
    ) -> Result<Self, String> {
        let started = Instant::now();
        #[cfg(rui_embedded_metallib)]
        if let Some(library) = embedded_library(device, name, source, functions) {
            return Ok(Self {
                library,
                origin: Origin::Embedded,
                load_time: started.elapsed(),
                archive: None,
                store_at: None,
                name: name.to_string(),
            });
        }
        Self::load_in(
            device,
            cache_dir().as_deref(),
            name,
            source,
            functions,
            started,
        )
    }

    /// Like [`Self::load`] with the archive under `dir`, or none.
    fn load_in(
        device: &Device,
        dir: Option<&Path>,
        name: &str,
        source: &str,
        functions: &[&str],
        started: Instant,
    ) -> Result<Self, String> {
        let file = dir.map(|dir| {
            let key = cache_key(source, device.name(), os_build());
            dir.join(format!("{name}-{key:016x}.metallib"))
        });
        if let Some(file) = &file {
            match cached_library(device, file, functions) {
                Ok((library, archive)) => {
                    return Ok(Self {
                        library,
                        origin: Origin::Cached,
                        load_time: started.elapsed(),
                        archive,
                        store_at: None,
                        name: name.to_string(),
                    });
                }
                Err(CacheMiss::Absent) => {
                    log::info!("no cached {name} shader at {}; compiling", file.display())
                }
                Err(CacheMiss::Unusable(reason)) => log::warn!(
                    "cached {name} shader at {} is unusable ({reason}); compiling it again",
                    file.display()
                ),
            }
        }
        let library = device.new_library_with_source(source, &CompileOptions::new())?;
        Ok(Self {
            library,
            origin: Origin::Compiled,
            load_time: started.elapsed(),
            archive: None,
            store_at: file,
            name: name.to_string(),
        })
    }

    pub(super) fn function(&self, name: &str) -> Result<Function, String> {
        self.library.get_function(name, None)
    }

    /// A pipeline for `desc`, looked up in this library's archive first.
    pub(super) fn pipeline(
        &self,
        device: &Device,
        desc: &RenderPipelineDescriptorRef,
    ) -> Result<RenderPipelineState, String> {
        if let Some(archive) = &self.archive {
            desc.set_binary_archives(&[archive]);
        }
        device.new_render_pipeline_state(desc)
    }

    /// `name=origin:load+pipelines` milliseconds, for the `RUI_PROFILE` line.
    pub(super) fn report(&self, pipelines: Duration) -> String {
        format!(
            "{}={}:{:.1}+{:.1}ms",
            self.name,
            self.origin.label(),
            self.load_time.as_secs_f64() * 1e3,
            pipelines.as_secs_f64() * 1e3
        )
    }

    /// Writes the archive of `pipelines` for the next launch, when this library
    /// was compiled from source. Failing only costs that launch the compile.
    pub(super) fn store(&self, device: &Device, pipelines: &[&RenderPipelineDescriptorRef]) {
        let Some(file) = &self.store_at else {
            return;
        };
        if let Err(err) = store_archive(device, file, pipelines) {
            log::warn!(
                "could not cache the {} shader at {}: {err}",
                self.name,
                file.display()
            );
        }
    }
}

/// The cache file name's key: the source, the device it compiles for and the
/// OS build, whose compiler made it.
fn cache_key(source: &str, device: &str, os_build: &str) -> u64 {
    shader_hash(&[
        CACHE_FORMAT,
        source.as_bytes(),
        device.as_bytes(),
        os_build.as_bytes(),
    ])
}

/// `$RUI_SHADER_CACHE_DIR`, or the user's cache directory.
fn cache_dir() -> Option<PathBuf> {
    match std::env::var_os(RUI_SHADER_CACHE_DIR_ENV) {
        Some(dir) if dir.is_empty() => None,
        Some(dir) => Some(PathBuf::from(dir)),
        None => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Caches/rui/shaders")),
    }
}

/// The OS build, such as `25F84`; empty if it cannot be read.
fn os_build() -> &'static str {
    static BUILD: OnceLock<String> = OnceLock::new();
    BUILD.get_or_init(|| {
        std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
            .ok()
            .and_then(|plist| plist_string(&plist, "ProductBuildVersion"))
            .unwrap_or_default()
    })
}

/// The `<string>` value after `<key>key</key>` in a property list.
fn plist_string(plist: &str, key: &str) -> Option<String> {
    let after_key = &plist[plist.find(&format!("<key>{key}</key>"))?..];
    let start = after_key.find("<string>")? + "<string>".len();
    let end = after_key[start..].find("</string>")? + start;
    Some(after_key[start..end].trim().to_string())
}

enum CacheMiss {
    Absent,
    Unusable(String),
}

fn cached_library(
    device: &Device,
    file: &Path,
    functions: &[&str],
) -> Result<(Library, Option<BinaryArchive>), CacheMiss> {
    if !file.exists() {
        return Err(CacheMiss::Absent);
    }
    let library = device
        .new_library_with_file(file)
        .map_err(CacheMiss::Unusable)?;
    let names = library.function_names();
    if let Some(missing) = functions
        .iter()
        .find(|function| !names.iter().any(|name| name == *function))
    {
        return Err(CacheMiss::Unusable(format!("it has no {missing} function")));
    }
    // Without the archive the pipelines still build, compiling GPU code if the
    // system's cache lacks it.
    let desc = BinaryArchiveDescriptor::new();
    desc.set_url(&file_url(file));
    let archive = device.new_binary_archive_with_descriptor(&desc).ok();
    Ok((library, archive))
}

fn store_archive(
    device: &Device,
    file: &Path,
    pipelines: &[&RenderPipelineDescriptorRef],
) -> Result<(), String> {
    let dir = file.parent().ok_or("the cache file has no directory")?;
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let archive = device.new_binary_archive_with_descriptor(&BinaryArchiveDescriptor::new())?;
    for desc in pipelines {
        archive.add_render_pipeline_functions_with_descriptor(desc)?;
    }
    // Written aside and renamed, so a reader never sees half a file.
    let file_name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let partial = dir.join(format!(".{file_name}.{}.partial", std::process::id()));
    archive.serialize_to_url(&file_url(&partial))?;
    std::fs::rename(&partial, file).map_err(|err| {
        let _ = std::fs::remove_file(&partial);
        err.to_string()
    })?;
    remove_stale(dir, file_name);
    Ok(())
}

/// Removes the archives of this shader's earlier sources.
fn remove_stale(dir: &Path, current: &str) {
    let Some((name, _)) = current.rsplit_once('-') else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let stale = file_name != current
            && file_name.ends_with(".metallib")
            && file_name.rsplit_once('-').map(|(prefix, _)| prefix) == Some(name);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// A `file://` URL for `path`, percent-encoding what a URL cannot hold.
fn file_url(path: &Path) -> ManuallyDrop<URL> {
    let mut url = String::from("file://");
    for &byte in path.to_string_lossy().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    // `URLWithString:` returns an autoreleased URL, which dropping a `URL`
    // would release a second time.
    ManuallyDrop::new(URL::new_with_string(&url))
}

/// The embedded library named `name`, if the build embedded one compiled from
/// this `source`.
#[cfg(rui_embedded_metallib)]
fn embedded_library(
    device: &Device,
    name: &str,
    source: &str,
    functions: &[&str],
) -> Option<Library> {
    let (hash, bytes) = embedded::library(name)?;
    if hash != shader_hash(&[source.as_bytes()]) {
        log::warn!("the embedded {name} shader was built from other source; not using it");
        return None;
    }
    let library = device
        .new_library_with_data(bytes)
        .map_err(|err| log::warn!("the embedded {name} shader does not load: {err}"))
        .ok()?;
    let names = library.function_names();
    functions
        .iter()
        .all(|function| names.iter().any(|name| name == *function))
        .then_some(library)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;
vertex float4 probe_vertex(uint id [[vertex_id]]) { return float4(float(id), 0.0, 0.0, 1.0); }
fragment float4 probe_fragment() { return float4(1.0); }
"#;
    const FUNCTIONS: &[&str] = &["probe_vertex", "probe_fragment"];

    #[test]
    fn key_changes_with_source_device_and_os_build() {
        let key = cache_key(SOURCE, "Apple M2 Max", "25F84");
        assert_eq!(key, cache_key(SOURCE, "Apple M2 Max", "25F84"));
        assert_ne!(
            key,
            cache_key(&format!("{SOURCE} "), "Apple M2 Max", "25F84")
        );
        assert_ne!(key, cache_key(SOURCE, "Apple M3 Max", "25F84"));
        assert_ne!(key, cache_key(SOURCE, "Apple M2 Max", "25F85"));
        // Parts do not run into each other.
        assert_ne!(cache_key("ab", "c", ""), cache_key("a", "bc", ""));
    }

    #[test]
    fn reads_the_os_build_from_a_property_list() {
        let plist = "<dict>\n\t<key>ProductBuildVersion</key>\n\t<string>25F84</string>\n\t<key>ProductVersion</key>\n\t<string>26.5.2</string>\n</dict>";
        assert_eq!(
            plist_string(plist, "ProductBuildVersion").as_deref(),
            Some("25F84")
        );
        assert_eq!(plist_string(plist, "Missing"), None);
    }

    fn device() -> Option<Device> {
        Device::system_default()
    }

    fn scratch_dir(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rui-shader-cache-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// Loads the probe library from `dir` as a launch would, storing it after
    /// a compile; returns where it came from.
    fn launch(device: &Device, dir: &Path, source: &str) -> Origin {
        let library = ShaderLibrary::load_in(
            device,
            Some(dir),
            "probe",
            source,
            FUNCTIONS,
            Instant::now(),
        )
        .expect("the probe shader loads");
        let desc = RenderPipelineDescriptor::new();
        desc.set_vertex_function(Some(&library.function("probe_vertex").unwrap()));
        desc.set_fragment_function(Some(&library.function("probe_fragment").unwrap()));
        desc.color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm_sRGB);
        library
            .pipeline(device, &desc)
            .expect("the probe pipeline builds");
        library.store(device, &[&desc]);
        library.origin
    }

    fn archives(dir: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default();
        files.sort();
        files
    }

    #[test]
    fn second_launch_loads_the_archive_the_first_wrote() {
        let Some(device) = device() else { return };
        let dir = scratch_dir("reuse");
        assert_eq!(launch(&device, &dir, SOURCE), Origin::Compiled);
        assert_eq!(archives(&dir).len(), 1);
        assert_eq!(launch(&device, &dir, SOURCE), Origin::Cached);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_archive_is_compiled_again_and_rewritten() {
        let Some(device) = device() else { return };
        let dir = scratch_dir("corrupt");
        launch(&device, &dir, SOURCE);
        let file = archives(&dir).pop().unwrap();
        std::fs::write(&file, b"not a metal library").unwrap();
        assert_eq!(launch(&device, &dir, SOURCE), Origin::Compiled);
        assert_eq!(launch(&device, &dir, SOURCE), Origin::Cached);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn archive_missing_a_function_is_compiled_again() {
        let Some(device) = device() else { return };
        let dir = scratch_dir("mismatch");
        launch(&device, &dir, SOURCE);
        let file = archives(&dir).pop().unwrap();
        // Another shader's archive under this one's name.
        let other = scratch_dir("mismatch-other");
        let renamed = SOURCE.replace("probe_fragment", "other_fragment");
        let library = device
            .new_library_with_source(&renamed, &CompileOptions::new())
            .unwrap();
        let desc = RenderPipelineDescriptor::new();
        desc.set_vertex_function(Some(&library.get_function("probe_vertex", None).unwrap()));
        desc.set_fragment_function(Some(&library.get_function("other_fragment", None).unwrap()));
        desc.color_attachments()
            .object_at(0)
            .unwrap()
            .set_pixel_format(MTLPixelFormat::BGRA8Unorm_sRGB);
        store_archive(&device, &other.join("x-0.metallib"), &[&desc]).unwrap();
        std::fs::rename(other.join("x-0.metallib"), &file).unwrap();
        assert_eq!(launch(&device, &dir, SOURCE), Origin::Compiled);
        assert_eq!(launch(&device, &dir, SOURCE), Origin::Cached);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn changed_source_compiles_and_replaces_the_stale_archive() {
        let Some(device) = device() else { return };
        let dir = scratch_dir("stale");
        launch(&device, &dir, SOURCE);
        let before = archives(&dir);
        let changed = format!("{SOURCE}// changed\n");
        assert_eq!(launch(&device, &dir, &changed), Origin::Compiled);
        let after = archives(&dir);
        assert_eq!(after.len(), 1, "the stale archive is removed");
        assert_ne!(before, after);
        assert_eq!(launch(&device, &dir, &changed), Origin::Cached);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
