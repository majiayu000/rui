use crate::ImageSource;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::Corners;
use crate::renderer::Primitive;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveSnapshot {
    text: String,
}

impl PrimitiveSnapshot {
    pub fn new(text: String) -> Self {
        Self { text }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for PrimitiveSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveSnapshotError {
    UnsupportedImageSource { index: usize, source: &'static str },
    Io { path: PathBuf, message: String },
    Mismatch { path: Option<PathBuf>, diff: String },
}

impl fmt::Display for PrimitiveSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedImageSource { index, source } => write!(
                f,
                "primitive snapshot does not support {source} image source at primitive {index}"
            ),
            Self::Io { path, message } => {
                write!(
                    f,
                    "primitive snapshot I/O failed for {}: {message}",
                    path.display()
                )
            }
            Self::Mismatch { path, diff } => {
                if let Some(path) = path {
                    write!(
                        f,
                        "primitive snapshot mismatch for {}:\n{diff}",
                        path.display()
                    )
                } else {
                    write!(f, "primitive snapshot mismatch:\n{diff}")
                }
            }
        }
    }
}

impl Error for PrimitiveSnapshotError {}

pub fn primitive_snapshot(
    primitives: &[Primitive],
) -> Result<PrimitiveSnapshot, PrimitiveSnapshotError> {
    let mut lines = Vec::with_capacity(primitives.len());
    for (index, primitive) in primitives.iter().enumerate() {
        lines.push(primitive_line(index, primitive)?);
    }
    Ok(PrimitiveSnapshot::new(lines.join("\n")))
}

pub fn assert_primitive_snapshot_text(
    actual: &PrimitiveSnapshot,
    expected: &str,
) -> Result<(), PrimitiveSnapshotError> {
    if actual.as_str() == expected {
        return Ok(());
    }

    Err(PrimitiveSnapshotError::Mismatch {
        path: None,
        diff: snapshot_diff(expected, actual.as_str()),
    })
}

pub fn assert_primitive_snapshot_file(
    path: impl AsRef<Path>,
    primitives: &[Primitive],
) -> Result<(), PrimitiveSnapshotError> {
    let path = path.as_ref();
    let actual = primitive_snapshot(primitives)?;

    if should_update_snapshots() {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|err| PrimitiveSnapshotError::Io {
                path: parent.to_path_buf(),
                message: err.to_string(),
            })?;
        }
        std::fs::write(path, actual.as_str()).map_err(|err| PrimitiveSnapshotError::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;
        return Ok(());
    }

    let expected = std::fs::read_to_string(path).map_err(|err| PrimitiveSnapshotError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    if expected == actual.as_str() {
        Ok(())
    } else {
        Err(PrimitiveSnapshotError::Mismatch {
            path: Some(path.to_path_buf()),
            diff: snapshot_diff(&expected, actual.as_str()),
        })
    }
}

fn primitive_line(index: usize, primitive: &Primitive) -> Result<String, PrimitiveSnapshotError> {
    let line = match primitive {
        Primitive::Quad {
            bounds,
            background,
            border_color,
            border_widths,
            corner_radii,
        } => format!(
            "primitive[{index}].quad bounds={} background={} border={} border_widths={} radii={}",
            bounds_text(*bounds),
            rgba_text(*background),
            rgba_text(*border_color),
            edges_text(*border_widths),
            corners_text(*corner_radii)
        ),
        Primitive::Shadow {
            bounds,
            corner_radii,
            blur_radius,
            color,
        } => format!(
            "primitive[{index}].shadow bounds={} blur={} color={} radii={}",
            bounds_text(*bounds),
            f32_text(*blur_radius),
            rgba_text(*color),
            corners_text(*corner_radii)
        ),
        Primitive::LinearGradient {
            bounds,
            start,
            end,
            angle,
            border_color,
            border_widths,
            corner_radii,
        } => format!(
            "primitive[{index}].linear_gradient bounds={} start={} end={} angle={} border={} border_widths={} radii={}",
            bounds_text(*bounds),
            rgba_text(*start),
            rgba_text(*end),
            f32_text(*angle),
            rgba_text(*border_color),
            edges_text(*border_widths),
            corners_text(*corner_radii)
        ),
        Primitive::RadialGradient {
            bounds,
            inner,
            outer,
            border_color,
            border_widths,
            corner_radii,
        } => format!(
            "primitive[{index}].radial_gradient bounds={} inner={} outer={} border={} border_widths={} radii={}",
            bounds_text(*bounds),
            rgba_text(*inner),
            rgba_text(*outer),
            rgba_text(*border_color),
            edges_text(*border_widths),
            corners_text(*corner_radii)
        ),
        Primitive::Text {
            bounds,
            content,
            color,
            font_size,
            font_weight,
            font_family,
            line_height,
            align,
        } => format!(
            "primitive[{index}].text bounds={} content=\"{}\" color={} font_size={} weight={} family={} line_height={} align={:?}",
            bounds_text(*bounds),
            escaped(content),
            rgba_text(*color),
            f32_text(*font_size),
            font_weight,
            option_text(font_family.as_deref()),
            f32_text(*line_height),
            align
        ),
        Primitive::Image {
            bounds,
            source,
            fit,
            corner_radii,
            opacity,
        } => match source {
            ImageSource::Data {
                data,
                width,
                height,
            } => format!(
                "primitive[{index}].image bounds={} source=data(width={},height={},len={},fnv64={}) fit={:?} opacity={} radii={}",
                bounds_text(*bounds),
                width,
                height,
                data.len(),
                fnv64_text(data),
                fit,
                f32_text(*opacity),
                corners_text(*corner_radii)
            ),
            ImageSource::File(_) => {
                return Err(PrimitiveSnapshotError::UnsupportedImageSource {
                    index,
                    source: "file",
                });
            }
            ImageSource::Texture(_) => {
                return Err(PrimitiveSnapshotError::UnsupportedImageSource {
                    index,
                    source: "texture",
                });
            }
        },
        Primitive::Path {
            vertices,
            color,
            stroke_width,
        } => {
            let points = vertices
                .iter()
                .map(|vertex| {
                    format!(
                        "({}, {})",
                        f32_text(vertex.position[0]),
                        f32_text(vertex.position[1])
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let stroke = match stroke_width {
                Some(width) => f32_text(*width),
                None => String::from("none"),
            };
            format!(
                "primitive[{index}].path vertices=[{}] color={} stroke_width={}",
                points,
                rgba_text(*color),
                stroke
            )
        }
        Primitive::PushClip {
            bounds,
            corner_radii,
        } => format!(
            "primitive[{index}].push_clip bounds={} radii={}",
            bounds_text(*bounds),
            corners_text(*corner_radii)
        ),
        Primitive::PopClip => format!("primitive[{index}].pop_clip"),
    };

    Ok(line)
}

fn snapshot_diff(expected: &str, actual: &str) -> String {
    let expected_lines = expected.lines().collect::<Vec<_>>();
    let actual_lines = actual.lines().collect::<Vec<_>>();
    let max_len = expected_lines.len().max(actual_lines.len());
    let mut diff = String::new();

    for index in 0..max_len {
        let expected_line = expected_lines.get(index).copied();
        let actual_line = actual_lines.get(index).copied();
        if expected_line == actual_line {
            continue;
        }

        diff.push_str(&format!("line {}:\n", index + 1));
        diff.push_str("- ");
        diff.push_str(line_or_missing(expected_line));
        diff.push('\n');
        diff.push_str("+ ");
        diff.push_str(line_or_missing(actual_line));
        diff.push('\n');
    }

    diff
}

fn line_or_missing(line: Option<&str>) -> &str {
    match line {
        Some(line) => line,
        None => "<missing>",
    }
}

fn should_update_snapshots() -> bool {
    match std::env::var("RUI_UPDATE_SNAPSHOTS") {
        Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    }
}

fn bounds_text(bounds: Bounds) -> String {
    format!(
        "({}, {}, {}, {})",
        f32_text(bounds.x()),
        f32_text(bounds.y()),
        f32_text(bounds.width()),
        f32_text(bounds.height())
    )
}

fn edges_text(edges: Edges) -> String {
    format!(
        "({}, {}, {}, {})",
        f32_text(edges.top),
        f32_text(edges.right),
        f32_text(edges.bottom),
        f32_text(edges.left)
    )
}

fn corners_text(corners: Corners) -> String {
    format!(
        "({}, {}, {}, {})",
        f32_text(corners.top_left),
        f32_text(corners.top_right),
        f32_text(corners.bottom_right),
        f32_text(corners.bottom_left)
    )
}

fn rgba_text(rgba: Rgba) -> String {
    format!(
        "rgba({}, {}, {}, {})",
        f32_text(rgba.r),
        f32_text(rgba.g),
        f32_text(rgba.b),
        f32_text(rgba.a)
    )
}

fn f32_text(value: f32) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    format!("{value:.3}")
}

fn escaped(value: &str) -> String {
    value.escape_default().collect()
}

fn option_text(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", escaped(value)),
        None => String::from("none"),
    }
}

fn fnv64_text(data: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("0x{hash:016x}")
}
