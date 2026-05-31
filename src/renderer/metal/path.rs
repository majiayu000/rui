use crate::core::color::Rgba;
use crate::renderer::RendererError;
use crate::renderer::primitives::PathVertex;
use bytemuck::{Pod, Zeroable};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct GpuPathVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl GpuPathVertex {
    fn new(position: [f32; 2], color: Rgba) -> Self {
        Self {
            position,
            color: color.to_array(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathMesh {
    pub vertices: Vec<GpuPathVertex>,
}

pub fn build_path_mesh(
    vertices: &[PathVertex],
    color: Rgba,
    stroke_width: Option<f32>,
) -> Result<PathMesh, RendererError> {
    match stroke_width {
        Some(width) => stroke_path(vertices, color, width),
        None => fill_path(vertices, color),
    }
}

fn fill_path(vertices: &[PathVertex], color: Rgba) -> Result<PathMesh, RendererError> {
    if vertices.len() < 3 {
        return Err(RendererError::render_failed(
            "filled path primitive requires at least three vertices",
        ));
    }

    let mut triangles = Vec::with_capacity((vertices.len() - 2) * 3);
    let origin = vertices[0].position;
    for index in 1..vertices.len() - 1 {
        triangles.push(GpuPathVertex::new(origin, color));
        triangles.push(GpuPathVertex::new(vertices[index].position, color));
        triangles.push(GpuPathVertex::new(vertices[index + 1].position, color));
    }

    Ok(PathMesh {
        vertices: triangles,
    })
}

fn stroke_path(
    vertices: &[PathVertex],
    color: Rgba,
    stroke_width: f32,
) -> Result<PathMesh, RendererError> {
    if stroke_width <= 0.0 || !stroke_width.is_finite() {
        return Err(RendererError::render_failed(
            "stroked path primitive requires a positive finite stroke width",
        ));
    }
    if vertices.len() < 2 {
        return Err(RendererError::render_failed(
            "stroked path primitive requires at least two vertices",
        ));
    }

    let mut triangles = Vec::with_capacity((vertices.len() - 1) * 6);
    let half_width = stroke_width * 0.5;

    for segment in vertices.windows(2) {
        let start = segment[0].position;
        let end = segment[1].position;
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = (dx * dx + dy * dy).sqrt();
        if length <= f32::EPSILON {
            continue;
        }

        let nx = -dy / length * half_width;
        let ny = dx / length * half_width;

        let a = [start[0] + nx, start[1] + ny];
        let b = [start[0] - nx, start[1] - ny];
        let c = [end[0] + nx, end[1] + ny];
        let d = [end[0] - nx, end[1] - ny];

        triangles.push(GpuPathVertex::new(a, color));
        triangles.push(GpuPathVertex::new(b, color));
        triangles.push(GpuPathVertex::new(c, color));
        triangles.push(GpuPathVertex::new(b, color));
        triangles.push(GpuPathVertex::new(d, color));
        triangles.push(GpuPathVertex::new(c, color));
    }

    if triangles.is_empty() {
        return Err(RendererError::render_failed(
            "stroked path primitive contains only degenerate segments",
        ));
    }

    Ok(PathMesh {
        vertices: triangles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(x: f32, y: f32) -> PathVertex {
        PathVertex::new(x, y)
    }

    #[test]
    fn fill_path_triangulates_polygon_fan() {
        let mesh = build_path_mesh(
            &[
                vertex(0.0, 0.0),
                vertex(10.0, 0.0),
                vertex(10.0, 10.0),
                vertex(0.0, 10.0),
            ],
            Rgba::WHITE,
            None,
        )
        .expect("filled path should tessellate");

        assert_eq!(mesh.vertices.len(), 6);
    }

    #[test]
    fn stroke_path_expands_segments_into_triangles() {
        let mesh = build_path_mesh(
            &[vertex(0.0, 0.0), vertex(10.0, 0.0)],
            Rgba::WHITE,
            Some(2.0),
        )
        .expect("stroked path should tessellate");

        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.vertices[0].position, [0.0, 1.0]);
        assert_eq!(mesh.vertices[1].position, [0.0, -1.0]);
    }

    #[test]
    fn invalid_paths_fail_explicitly() {
        assert!(build_path_mesh(&[vertex(0.0, 0.0)], Rgba::WHITE, None).is_err());
        assert!(build_path_mesh(&[vertex(0.0, 0.0)], Rgba::WHITE, Some(1.0)).is_err());
        assert!(
            build_path_mesh(
                &[vertex(0.0, 0.0), vertex(1.0, 1.0)],
                Rgba::WHITE,
                Some(0.0),
            )
            .is_err()
        );
    }
}
