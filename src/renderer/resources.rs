//! Renderer-owned resource lifecycle management.

use crate::ImageSource;
use crate::core::geometry::Size;
use crate::renderer::primitives::PrimitiveKind;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererResourceKind {
    Texture,
    Glyph,
    Image,
}

impl RendererResourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Glyph => "glyph",
            Self::Image => "image",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererResourceId(pub u32);

impl RendererResourceId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererResourceHandle {
    pub id: RendererResourceId,
    pub kind: RendererResourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendererResourceError {
    InvalidResource {
        kind: RendererResourceKind,
        message: String,
    },
    MissingResource {
        kind: RendererResourceKind,
        id: u32,
    },
    UnsupportedResource {
        kind: RendererResourceKind,
        message: String,
    },
    ResourcePressure {
        kind: RendererResourceKind,
        requested_bytes: usize,
        max_bytes: usize,
        active_bytes: usize,
    },
}

impl RendererResourceError {
    pub fn invalid(kind: RendererResourceKind, message: impl Into<String>) -> Self {
        Self::InvalidResource {
            kind,
            message: message.into(),
        }
    }

    pub fn missing(kind: RendererResourceKind, id: u32) -> Self {
        Self::MissingResource { kind, id }
    }

    pub fn unsupported(kind: RendererResourceKind, message: impl Into<String>) -> Self {
        Self::UnsupportedResource {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> RendererResourceKind {
        match self {
            Self::InvalidResource { kind, .. }
            | Self::MissingResource { kind, .. }
            | Self::UnsupportedResource { kind, .. }
            | Self::ResourcePressure { kind, .. } => *kind,
        }
    }

    pub fn resource_id(&self) -> Option<u32> {
        match self {
            Self::MissingResource { id, .. } if *id != 0 => Some(*id),
            _ => None,
        }
    }

    pub fn is_pressure(&self) -> bool {
        matches!(self, Self::ResourcePressure { .. })
    }
}

impl fmt::Display for RendererResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidResource { kind, message } => {
                write!(f, "invalid {} resource: {message}", kind.name())
            }
            Self::MissingResource { kind, id } => {
                write!(f, "missing {} resource with id {id}", kind.name())
            }
            Self::UnsupportedResource { kind, message } => {
                write!(f, "unsupported {} resource: {message}", kind.name())
            }
            Self::ResourcePressure {
                kind,
                requested_bytes,
                max_bytes,
                active_bytes,
            } => write!(
                f,
                "{} resource pressure: requested {requested_bytes} bytes, max {max_bytes} bytes, active {active_bytes} bytes",
                kind.name()
            ),
        }
    }
}

impl Error for RendererResourceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererResourceStats {
    pub kind: RendererResourceKind,
    pub live_entries: usize,
    pub live_bytes: usize,
    pub disposed_entries: usize,
    pub evicted_entries: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub pressure_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererResourceLimits {
    pub texture_max_entries: usize,
    pub texture_max_bytes: usize,
    pub image_max_entries: usize,
    pub image_max_bytes: usize,
    pub glyph_max_entries: usize,
    pub glyph_max_bytes: usize,
}

impl RendererResourceLimits {
    pub const fn unbounded() -> Self {
        Self {
            texture_max_entries: usize::MAX,
            texture_max_bytes: usize::MAX,
            image_max_entries: usize::MAX,
            image_max_bytes: usize::MAX,
            glyph_max_entries: usize::MAX,
            glyph_max_bytes: usize::MAX,
        }
    }
}

impl Default for RendererResourceLimits {
    fn default() -> Self {
        Self::unbounded()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererDeviceDiagnostics {
    pub backend: String,
    pub device_name: String,
    pub is_headless: bool,
    pub unified_memory: Option<bool>,
    pub recommended_max_working_set_size: Option<u64>,
}

impl RendererDeviceDiagnostics {
    pub fn headless(backend: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
            device_name: String::from("headless"),
            is_headless: true,
            unified_memory: None,
            recommended_max_working_set_size: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererUnsupportedPrimitive {
    pub backend: String,
    pub primitive: PrimitiveKind,
    pub reason: String,
}

impl RendererUnsupportedPrimitive {
    pub fn new(
        backend: impl Into<String>,
        primitive: PrimitiveKind,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            backend: backend.into(),
            primitive,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererDiagnostics {
    pub device: RendererDeviceDiagnostics,
    pub resources: Vec<RendererResourceStats>,
    pub unsupported_primitives: Vec<RendererUnsupportedPrimitive>,
}

impl RendererDiagnostics {
    pub fn new(device: RendererDeviceDiagnostics, resources: Vec<RendererResourceStats>) -> Self {
        Self {
            device,
            resources,
            unsupported_primitives: Vec::new(),
        }
    }

    pub fn headless(backend: impl Into<String>) -> Self {
        Self {
            device: RendererDeviceDiagnostics::headless(backend),
            resources: Vec::new(),
            unsupported_primitives: Vec::new(),
        }
    }

    pub fn with_unsupported_primitives(
        mut self,
        unsupported_primitives: Vec<RendererUnsupportedPrimitive>,
    ) -> Self {
        self.unsupported_primitives = unsupported_primitives;
        self
    }

    pub fn resource(&self, kind: RendererResourceKind) -> Option<&RendererResourceStats> {
        self.resources.iter().find(|stats| stats.kind == kind)
    }

    pub fn unsupported_primitive(
        &self,
        primitive: PrimitiveKind,
    ) -> Option<&RendererUnsupportedPrimitive> {
        self.unsupported_primitives
            .iter()
            .find(|unsupported| unsupported.primitive == primitive)
    }

    pub fn total_live_entries(&self) -> usize {
        self.resources.iter().map(|stats| stats.live_entries).sum()
    }

    pub fn total_live_bytes(&self) -> usize {
        self.resources.iter().map(|stats| stats.live_bytes).sum()
    }

    pub fn total_pressure_events(&self) -> usize {
        self.resources
            .iter()
            .map(|stats| stats.pressure_events)
            .sum()
    }

    pub fn total_evicted_entries(&self) -> usize {
        self.resources
            .iter()
            .map(|stats| stats.evicted_entries)
            .sum()
    }

    pub fn total_cache_hits(&self) -> usize {
        self.resources.iter().map(|stats| stats.cache_hits).sum()
    }

    pub fn total_cache_misses(&self) -> usize {
        self.resources.iter().map(|stats| stats.cache_misses).sum()
    }
}

#[derive(Debug, Clone)]
pub struct RendererResourceAllocation<K> {
    pub handle: RendererResourceHandle,
    pub evicted: Vec<K>,
    pub reused: bool,
}

#[derive(Debug, Clone)]
struct RendererResourceRecord {
    handle: RendererResourceHandle,
    byte_size: usize,
    last_used: u64,
    active: bool,
}

#[derive(Debug, Clone)]
pub struct RendererResourceCache<K>
where
    K: Clone + Eq + Hash,
{
    kind: RendererResourceKind,
    max_entries: usize,
    max_bytes: usize,
    next_id: u32,
    tick: u64,
    live_bytes: usize,
    pressure_events: usize,
    disposed_entries: usize,
    evicted_entries: usize,
    cache_hits: usize,
    cache_misses: usize,
    entries: HashMap<K, RendererResourceRecord>,
}

impl<K> RendererResourceCache<K>
where
    K: Clone + Eq + Hash,
{
    pub fn new(kind: RendererResourceKind, max_entries: usize, max_bytes: usize) -> Self {
        Self {
            kind,
            max_entries,
            max_bytes,
            next_id: 1,
            tick: 0,
            live_bytes: 0,
            pressure_events: 0,
            disposed_entries: 0,
            evicted_entries: 0,
            cache_hits: 0,
            cache_misses: 0,
            entries: HashMap::new(),
        }
    }

    pub fn unbounded(kind: RendererResourceKind) -> Self {
        Self::new(kind, usize::MAX, usize::MAX)
    }

    pub fn begin_frame(&mut self) {
        for record in self.entries.values_mut() {
            record.active = false;
        }
    }

    pub fn resolve(
        &mut self,
        key: K,
        byte_size: usize,
    ) -> Result<RendererResourceAllocation<K>, RendererResourceError> {
        if byte_size == 0 {
            return Err(RendererResourceError::invalid(
                self.kind,
                "resource byte size must be greater than zero",
            ));
        }

        self.tick += 1;
        if let Some(record) = self.entries.get_mut(&key) {
            self.cache_hits += 1;
            record.last_used = self.tick;
            record.active = true;
            return Ok(RendererResourceAllocation {
                handle: record.handle,
                evicted: Vec::new(),
                reused: true,
            });
        }

        self.cache_misses += 1;
        let evicted = self.make_room_for(byte_size)?;
        let handle = RendererResourceHandle {
            id: RendererResourceId(self.next_id),
            kind: self.kind,
        };
        self.next_id += 1;
        self.live_bytes += byte_size;
        self.entries.insert(
            key,
            RendererResourceRecord {
                handle,
                byte_size,
                last_used: self.tick,
                active: true,
            },
        );

        Ok(RendererResourceAllocation {
            handle,
            evicted,
            reused: false,
        })
    }

    pub fn dispose(&mut self, key: &K) -> Result<RendererResourceHandle, RendererResourceError> {
        let record = self
            .entries
            .remove(key)
            .ok_or_else(|| RendererResourceError::missing(self.kind, 0))?;
        self.live_bytes = self.live_bytes.saturating_sub(record.byte_size);
        self.disposed_entries += 1;
        Ok(record.handle)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    pub fn stats(&self) -> RendererResourceStats {
        RendererResourceStats {
            kind: self.kind,
            live_entries: self.entries.len(),
            live_bytes: self.live_bytes,
            disposed_entries: self.disposed_entries,
            evicted_entries: self.evicted_entries,
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            pressure_events: self.pressure_events,
        }
    }

    fn make_room_for(&mut self, byte_size: usize) -> Result<Vec<K>, RendererResourceError> {
        if self.max_entries == 0 || byte_size > self.max_bytes {
            self.pressure_events += 1;
            return Err(self.pressure_error(byte_size));
        }

        let mut evicted = Vec::new();
        while self.entries.len() + 1 > self.max_entries
            || self.live_bytes + byte_size > self.max_bytes
        {
            let Some(key) = self.next_evictable_key() else {
                self.pressure_events += 1;
                return Err(self.pressure_error(byte_size));
            };

            let record = match self.entries.remove(&key) {
                Some(record) => record,
                None => {
                    self.pressure_events += 1;
                    return Err(RendererResourceError::missing(self.kind, 0));
                }
            };
            self.live_bytes = self.live_bytes.saturating_sub(record.byte_size);
            self.disposed_entries += 1;
            self.evicted_entries += 1;
            evicted.push(key);
        }
        Ok(evicted)
    }

    fn next_evictable_key(&self) -> Option<K> {
        self.entries
            .iter()
            .filter(|(_, record)| !record.active)
            .min_by_key(|(_, record)| record.last_used)
            .map(|(key, _)| key.clone())
    }

    fn pressure_error(&self, requested_bytes: usize) -> RendererResourceError {
        let active_bytes = self
            .entries
            .values()
            .filter(|record| record.active)
            .map(|record| record.byte_size)
            .sum();
        RendererResourceError::ResourcePressure {
            kind: self.kind,
            requested_bytes,
            max_bytes: self.max_bytes,
            active_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageResourceKey {
    File(String),
    Data { hash: u64, width: u32, height: u32 },
}

impl ImageResourceKey {
    pub fn from_source(source: &ImageSource) -> Result<Self, RendererResourceError> {
        match source {
            ImageSource::File(path) => Ok(Self::File(path.clone())),
            ImageSource::Data {
                data,
                width,
                height,
            } => {
                let expected_len = (*width as usize) * (*height as usize) * 4;
                if data.len() != expected_len {
                    return Err(RendererResourceError::invalid(
                        RendererResourceKind::Image,
                        format!("expected {expected_len} RGBA bytes, got {}", data.len()),
                    ));
                }
                let mut hasher = DefaultHasher::new();
                data.hash(&mut hasher);
                Ok(Self::Data {
                    hash: hasher.finish(),
                    width: *width,
                    height: *height,
                })
            }
            ImageSource::Texture(id) => Err(RendererResourceError::unsupported(
                RendererResourceKind::Image,
                format!("external texture {id} is resolved by the renderer backend"),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageResourceEntry {
    pub handle: RendererResourceHandle,
    pub size: Size,
    pub pixels: Vec<u8>,
}

impl ImageResourceEntry {
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }
}

pub struct RendererImageCache {
    resources: RendererResourceCache<ImageResourceKey>,
    entries: HashMap<ImageResourceKey, Arc<ImageResourceEntry>>,
}

impl RendererImageCache {
    pub fn new() -> Self {
        Self {
            resources: RendererResourceCache::unbounded(RendererResourceKind::Image),
            entries: HashMap::new(),
        }
    }

    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            resources: RendererResourceCache::new(
                RendererResourceKind::Image,
                max_entries,
                max_bytes,
            ),
            entries: HashMap::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.resources.begin_frame();
    }

    pub fn resolve(
        &mut self,
        source: &ImageSource,
    ) -> Result<Arc<ImageResourceEntry>, RendererResourceError> {
        let key = ImageResourceKey::from_source(source)?;
        if let Some(entry) = self.entries.get(&key).cloned() {
            let allocation = self.resources.resolve(key.clone(), entry.byte_size())?;
            self.drop_evicted(allocation.evicted);
            return Ok(entry);
        }

        let (size, pixels) = load_image_pixels(source)?;
        let byte_size = pixels.len();
        let allocation = self.resources.resolve(key.clone(), byte_size)?;
        self.drop_evicted(allocation.evicted);

        let entry = Arc::new(ImageResourceEntry {
            handle: allocation.handle,
            size,
            pixels,
        });
        self.entries.insert(key, entry.clone());
        Ok(entry)
    }

    pub fn dispose(
        &mut self,
        key: &ImageResourceKey,
    ) -> Result<RendererResourceHandle, RendererResourceError> {
        self.entries.remove(key);
        self.resources.dispose(key)
    }

    pub fn stats(&self) -> RendererResourceStats {
        self.resources.stats()
    }

    fn drop_evicted(&mut self, evicted: Vec<ImageResourceKey>) {
        for key in evicted {
            self.entries.remove(&key);
        }
    }
}

impl Default for RendererImageCache {
    fn default() -> Self {
        Self::new()
    }
}

fn load_image_pixels(source: &ImageSource) -> Result<(Size, Vec<u8>), RendererResourceError> {
    match source {
        ImageSource::File(path) => {
            let image = image::open(path).map_err(|err| {
                RendererResourceError::invalid(
                    RendererResourceKind::Image,
                    format!("failed to load image {path}: {err}"),
                )
            })?;
            let rgba = image.to_rgba8();
            let (width, height) = rgba.dimensions();
            Ok((Size::new(width as f32, height as f32), rgba.into_raw()))
        }
        ImageSource::Data {
            data,
            width,
            height,
        } => Ok((Size::new(*width as f32, *height as f32), data.clone())),
        ImageSource::Texture(_) => {
            let _ = ImageResourceKey::from_source(source)?;
            Err(RendererResourceError::invalid(
                RendererResourceKind::Image,
                "unreachable image source state",
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphResourceKey {
    content: String,
    size_bits: u32,
    weight: u16,
    family: Option<String>,
    line_height_bits: u32,
}

impl GlyphResourceKey {
    pub fn new(
        content: impl Into<String>,
        font_size: f32,
        weight: u16,
        family: Option<&str>,
        line_height: f32,
    ) -> Self {
        Self {
            content: content.into(),
            size_bits: font_size.to_bits(),
            weight,
            family: family.map(str::to_string),
            line_height_bits: line_height.to_bits(),
        }
    }
}
