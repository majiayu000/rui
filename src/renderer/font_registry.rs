//! Faces added to the process font set after the built-in table: fonts an app
//! registers from bytes or a file, and installed fonts found by family name.
//!
//! They are kept here and appended to every load of the system font set, so
//! the table's faces keep their indices and the first face stays the default
//! family.

use crate::renderer::system_fonts::{FontLoadError, read_font_file, reload_system_fonts};
use crate::renderer::text::TextError;
use crate::renderer::text_shaping::{FontBytes, TextShapingFont};
use rustybuzz::ttf_parser::{PlatformId, RawFace, Tag, fonts_in_collection, name, name_id};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

/// Faces read from one collection at most; a larger count is a damaged header.
const MAX_FACES_PER_FILE: u32 = 256;

/// How deep font directories are searched.
const MAX_DIRECTORY_DEPTH: usize = 8;

struct AddedFonts {
    faces: Vec<TextShapingFont>,
    /// Installed families already looked up, found or not, lowercased.
    looked_up: Vec<String>,
}

static ADDED_FONTS: Mutex<AddedFonts> = Mutex::new(AddedFonts {
    faces: Vec::new(),
    looked_up: Vec::new(),
});

fn added_fonts() -> MutexGuard<'static, AddedFonts> {
    ADDED_FONTS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The faces registered or found so far, in the order they were added.
pub(crate) fn added_faces() -> Vec<TextShapingFont> {
    added_fonts().faces.clone()
}

/// Adds every face in `bytes`, a font file or collection, to the font set and
/// returns the families they were added under: each face's typographic family
/// name, else its family name. Text naming one of them then uses it.
///
/// Bytes from `include_bytes!` are used in place; owned bytes are kept.
/// Faces stay registered for the life of the process.
///
/// ```ignore
/// let families = rui::renderer::text::register_font(include_bytes!("Mono.ttf").as_slice())?;
/// assert_eq!(families, ["JetBrains Mono"]);
/// ```
pub fn register_font(bytes: impl Into<Cow<'static, [u8]>>) -> Result<Vec<String>, TextError> {
    let bytes = match bytes.into() {
        Cow::Borrowed(bytes) => FontBytes::Static(bytes),
        Cow::Owned(bytes) => FontBytes::Owned(bytes),
    };
    register(Arc::new(bytes), "font data")
}

/// [`register_font`] with the contents of the file at `path`.
pub fn register_font_file(path: impl AsRef<Path>) -> Result<Vec<String>, TextError> {
    let path = path.as_ref();
    let label = path.to_string_lossy();
    let bytes = read_font_file(&label).map_err(|error| FontLoadError::new(&label, error))?;
    register(Arc::new(bytes), &label)
}

fn register(data: Arc<FontBytes>, label: &str) -> Result<Vec<String>, TextError> {
    let count = face_count(data.as_slice())
        .ok_or_else(|| FontLoadError::invalid_data(label, "not a font file or collection"))?;
    let mut faces = Vec::new();
    for index in 0..count {
        let family = face_family(data.as_slice(), index).ok_or_else(|| {
            FontLoadError::invalid_data(label, format!("font face {index} has no family name"))
        })?;
        let face = TextShapingFont::new(family, index, Arc::clone(&data)).ok_or_else(|| {
            FontLoadError::invalid_data(label, format!("font face {index} cannot be parsed"))
        })?;
        faces.push(face);
    }
    let mut families: Vec<String> = Vec::new();
    for face in &faces {
        if !families.contains(&face.family) {
            families.push(face.family.clone());
        }
    }
    added_fonts().faces.extend(faces);
    reload_system_fonts();
    Ok(families)
}

/// Adds the installed faces of `family` to the font set, looking each family
/// up once. Whether any face was added; the caller reloads the font set then.
pub(crate) fn add_installed_family(family: &str) -> bool {
    let key = family.to_lowercase();
    let mut added = added_fonts();
    if added.looked_up.contains(&key) {
        return false;
    }
    added.looked_up.push(key.clone());
    let mut found = false;
    for entry in installed_faces().iter().filter(|entry| entry.key == key) {
        let label = entry.path.to_string_lossy();
        let Ok(bytes) = read_font_file(&label) else {
            continue;
        };
        if let Some(face) = TextShapingFont::new(entry.family.clone(), entry.index, Arc::new(bytes))
        {
            added.faces.push(face);
            found = true;
        }
    }
    found
}

struct InstalledFace {
    /// `family`, lowercased.
    key: String,
    family: String,
    path: PathBuf,
    index: u32,
}

/// Every face in the system and user font directories, read once.
fn installed_faces() -> &'static [InstalledFace] {
    static FACES: OnceLock<Vec<InstalledFace>> = OnceLock::new();
    FACES.get_or_init(|| {
        let mut files = Vec::new();
        for directory in font_directories() {
            collect_font_files(&directory, 0, &mut files);
        }
        let mut faces = Vec::new();
        for path in files {
            let Ok(bytes) = read_font_file(&path.to_string_lossy()) else {
                continue;
            };
            let data = bytes.as_slice();
            for index in 0..face_count(data).unwrap_or(0) {
                if let Some(family) = face_family(data, index) {
                    faces.push(InstalledFace {
                        key: family.to_lowercase(),
                        family,
                        path: path.clone(),
                        index,
                    });
                }
            }
        }
        faces
    })
}

#[cfg(target_os = "macos")]
fn font_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(Path::new(&home).join("Library/Fonts"));
    }
    directories
}

#[cfg(windows)]
fn font_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        Path::new(&std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()))
            .join("Fonts"),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        directories.push(Path::new(&local).join(r"Microsoft\Windows\Fonts"));
    }
    directories
}

#[cfg(not(any(target_os = "macos", windows)))]
fn font_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")));
    if let Some(data_home) = data_home {
        directories.push(data_home.join("fonts"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(Path::new(&home).join(".fonts"));
    }
    directories
}

fn collect_font_files(directory: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth < MAX_DIRECTORY_DEPTH {
                collect_font_files(&path, depth + 1, files);
            }
        } else if is_font_file(&path) {
            files.push(path);
        }
    }
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["ttf", "otf", "ttc", "otc"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        })
}

/// Faces in a font file: one, or a collection's count.
fn face_count(data: &[u8]) -> Option<u32> {
    match fonts_in_collection(data) {
        Some(count) => (1..=MAX_FACES_PER_FILE).contains(&count).then_some(count),
        None => RawFace::parse(data, 0).is_ok().then_some(1),
    }
}

/// Face `index`'s typographic family name, else its family name, in English
/// when the font has one.
pub(crate) fn face_family(data: &[u8], index: u32) -> Option<String> {
    let raw = RawFace::parse(data, index).ok()?;
    let table = name::Table::parse(raw.table(Tag::from_bytes(b"name"))?)?;
    let mut best: Option<((u8, bool), String)> = None;
    for record in table.names {
        let rank = match record.name_id {
            name_id::TYPOGRAPHIC_FAMILY => 0,
            name_id::FAMILY => 1,
            _ => continue,
        };
        let english = match record.platform_id {
            PlatformId::Windows => record.language_id == 0x0409,
            PlatformId::Macintosh => record.language_id == 0,
            _ => true,
        };
        let text = if record.is_unicode() {
            record.to_string()
        } else if record.platform_id == PlatformId::Macintosh && record.name.is_ascii() {
            String::from_utf8(record.name.to_vec()).ok()
        } else {
            None
        };
        let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
            continue;
        };
        let key = (rank, !english);
        if best.as_ref().is_none_or(|(best_key, _)| key < *best_key) {
            best = Some((key, text.trim().to_string()));
        }
    }
    best.map(|(_, family)| family)
}
