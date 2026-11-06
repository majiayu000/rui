//! Images decoded off the UI thread, kept while shown and within a byte budget.

use super::AppContext;
use crate::core::entity::EntityId;
use crate::elements::{DecodedImage, Image};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Decoded bytes kept by default for images nothing shows any more.
const DEFAULT_IMAGE_BUDGET: usize = 256 * 1024 * 1024;

/// An encoded image (PNG, JPEG, ...) for [`AppContext::image`] to decode.
///
/// Bytes are told apart by where they are, not by what they hold: keep the
/// `Arc` and pass clones of it, as a new `Arc` is decoded again.
#[derive(Debug, Clone)]
pub enum ImageAsset {
    Path(PathBuf),
    Static(&'static [u8]),
    Bytes(Arc<[u8]>),
}

impl ImageAsset {
    fn key(&self) -> AssetKey {
        match self {
            Self::Path(path) => AssetKey::Path(path.clone()),
            Self::Static(bytes) => AssetKey::Bytes(bytes.as_ptr() as usize, bytes.len()),
            Self::Bytes(bytes) => AssetKey::Bytes(bytes.as_ptr() as usize, bytes.len()),
        }
    }

    fn decode(&self) -> Result<DecodedImage, String> {
        let decoded = match self {
            Self::Path(path) => image::open(path)
                .map_err(|err| format!("failed to load image {}: {err}", path.display())),
            Self::Static(bytes) => decode_bytes(bytes),
            Self::Bytes(bytes) => decode_bytes(bytes),
        }?;
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        DecodedImage::new(width, height, rgba.into_raw())
    }
}

fn decode_bytes(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    image::load_from_memory(bytes)
        .map_err(|err| format!("failed to decode {} image bytes: {err}", bytes.len()))
}

impl From<&str> for ImageAsset {
    fn from(path: &str) -> Self {
        Self::Path(path.into())
    }
}

impl From<String> for ImageAsset {
    fn from(path: String) -> Self {
        Self::Path(path.into())
    }
}

impl From<PathBuf> for ImageAsset {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&'static [u8]> for ImageAsset {
    fn from(bytes: &'static [u8]) -> Self {
        Self::Static(bytes)
    }
}

impl From<Arc<[u8]>> for ImageAsset {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::Bytes(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AssetKey {
    Path(PathBuf),
    /// Address and length.
    Bytes(usize, usize),
}

#[derive(Clone)]
enum Load {
    Loading,
    Ready(Arc<DecodedImage>),
    Failed(Arc<str>),
}

struct AssetEntry {
    /// Read by whoever builds an image of it, notified when it loads.
    marker: EntityId,
    load: Load,
    last_used: u64,
    /// Built since it loaded. Until then its builder has not seen it, and
    /// evicting it would load it again when that builder runs.
    built: bool,
    /// Keeps the bytes, and so the address that keys them, alive.
    _asset: ImageAsset,
}

pub(crate) struct ImageAssets {
    entries: HashMap<AssetKey, AssetEntry>,
    budget: usize,
    tick: u64,
}

impl Default for ImageAssets {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            budget: DEFAULT_IMAGE_BUDGET,
            tick: 0,
        }
    }
}

impl AppContext {
    /// An image of `asset`, decoded off the UI thread.
    ///
    /// Until it is decoded the image shows its placeholder, and if it cannot
    /// be, its alt text or the error. Whatever built it (a view's render, or
    /// the root) builds again when it is ready; nothing else does, so only the
    /// image is painted again.
    pub fn image(&mut self, asset: impl Into<ImageAsset>) -> Image {
        let asset = asset.into();
        let key = asset.key();
        self.images.tick += 1;
        let tick = self.images.tick;
        if let Some(entry) = self.images.entries.get_mut(&key) {
            entry.last_used = tick;
            entry.built = true;
            let (marker, load) = (entry.marker, entry.load.clone());
            self.record_read(marker);
            return match load {
                Load::Loading => Image::loading(),
                Load::Ready(image) => Image::decoded(image),
                Load::Failed(error) => Image::failed(error),
            };
        }

        let marker = self.create(()).id();
        self.record_read(marker);
        let decode = asset.clone();
        self.images.entries.insert(
            key.clone(),
            AssetEntry {
                marker,
                load: Load::Loading,
                last_used: tick,
                built: false,
                _asset: asset,
            },
        );
        let decoded = self.background_spawn(async move { decode.decode() });
        self.spawn(move |cx| async move {
            let decoded = decoded.await;
            cx.update(|cx| cx.finish_image(&key, decoded));
        })
        .detach();
        Image::loading()
    }

    /// Keep at most `bytes` of decoded images, dropping the least recently
    /// built first. Images still shown are kept whatever the budget.
    pub fn set_image_budget(&mut self, bytes: usize) {
        self.images.budget = bytes;
        self.evict_images();
    }

    /// Bytes of decoded images kept.
    pub fn image_bytes(&self) -> usize {
        self.images
            .entries
            .values()
            .map(|entry| match &entry.load {
                Load::Ready(image) => image.pixels().len(),
                Load::Loading | Load::Failed(_) => 0,
            })
            .sum()
    }

    fn finish_image(&mut self, key: &AssetKey, decoded: Result<DecodedImage, String>) {
        let Some(entry) = self.images.entries.get_mut(key) else {
            return;
        };
        entry.built = false;
        entry.load = match decoded {
            Ok(image) => Load::Ready(Arc::new(image)),
            Err(error) => Load::Failed(error.into()),
        };
        let marker = entry.marker;
        self.evict_images();
        self.notify(marker);
    }

    fn evict_images(&mut self) {
        let mut bytes = self.image_bytes();
        while bytes > self.images.budget {
            // Only what was built and nothing holds now: no element or scene
            // shows it.
            let victim = self
                .images
                .entries
                .iter()
                .filter_map(|(key, entry)| match &entry.load {
                    Load::Ready(image) if entry.built && Arc::strong_count(image) == 1 => {
                        Some((entry.last_used, key.clone(), image.pixels().len()))
                    }
                    _ => None,
                })
                .min_by_key(|(last_used, _, _)| *last_used);
            let Some((_, key, len)) = victim else {
                return;
            };
            if let Some(entry) = self.images.entries.remove(&key) {
                self.entities.remove(entry.marker);
            }
            bytes -= len;
        }
    }
}

#[cfg(test)]
mod tests;
