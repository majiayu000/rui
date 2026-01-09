//! Metal shader source code

/// Metal Shading Language source for quad rendering
pub const QUAD_SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

// Vertex input
struct QuadVertex {
    float2 position [[attribute(0)]];
};

// Instance data (per quad)
struct QuadInstance {
    float4 bounds;           // x, y, width, height
    float4 background;       // r, g, b, a
    float4 border_color;     // r, g, b, a
    float4 border_widths;    // top, right, bottom, left
    float4 corner_radii;     // top_left, top_right, bottom_right, bottom_left
};

// Uniforms
struct Uniforms {
    float2 viewport_size;
};

// Vertex output / Fragment input
struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float4 background;
    float4 border_color;
    float4 border_widths;
    float4 corner_radii;
    float2 size;
};

// Convert from pixel coordinates to clip space
float4 to_clip_space(float2 position, float2 viewport_size) {
    float2 ndc = (position / viewport_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;  // Flip Y for Metal coordinate system
    return float4(ndc, 0.0, 1.0);
}

// Vertex shader
vertex VertexOut quad_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    constant QuadInstance* instances [[buffer(0)]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    // Unit quad vertices (two triangles)
    float2 positions[6] = {
        float2(0, 0), float2(1, 0), float2(0, 1),
        float2(1, 0), float2(1, 1), float2(0, 1)
    };

    QuadInstance instance = instances[instance_id];
    float2 unit_pos = positions[vertex_id];

    // Calculate pixel position
    float2 origin = instance.bounds.xy;
    float2 size = instance.bounds.zw;
    float2 pixel_pos = origin + unit_pos * size;

    VertexOut out;
    out.position = to_clip_space(pixel_pos, uniforms.viewport_size);
    out.uv = unit_pos;
    out.background = instance.background;
    out.border_color = instance.border_color;
    out.border_widths = instance.border_widths;
    out.corner_radii = instance.corner_radii;
    out.size = size;

    return out;
}

// Signed distance function for rounded rectangle
float rounded_rect_sdf(float2 point, float2 half_size, float radius) {
    float2 d = abs(point) - half_size + radius;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;
}

// Pick corner radius based on position
float pick_corner_radius(float2 uv, float4 radii) {
    if (uv.x < 0.5) {
        return uv.y < 0.5 ? radii.x : radii.w;  // top_left or bottom_left
    } else {
        return uv.y < 0.5 ? radii.y : radii.z;  // top_right or bottom_right
    }
}

// Fragment shader
fragment float4 quad_fragment(VertexOut in [[stage_in]]) {
    float2 size = in.size;
    float2 half_size = size / 2.0;
    float2 center = half_size;
    float2 point = in.uv * size;
    float2 rel_point = point - center;

    // Pick the appropriate corner radius
    float corner_radius = pick_corner_radius(in.uv, in.corner_radii);
    corner_radius = min(corner_radius, min(half_size.x, half_size.y));

    // SDF for outer edge
    float dist = rounded_rect_sdf(rel_point, half_size, corner_radius);

    // Antialiasing
    float aa = fwidth(dist);
    float alpha = 1.0 - smoothstep(-aa, aa, dist);

    if (alpha < 0.001) {
        discard_fragment();
    }

    // Border handling
    float border_width = max(max(in.border_widths.x, in.border_widths.y),
                             max(in.border_widths.z, in.border_widths.w));

    if (border_width > 0.0) {
        // Inner edge for border
        float2 inner_half_size = half_size - border_width;
        float inner_radius = max(0.0, corner_radius - border_width);
        float inner_dist = rounded_rect_sdf(rel_point, inner_half_size, inner_radius);

        float border_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);
        float in_border = alpha - border_alpha;

        // Mix border and background
        float4 color = mix(in.background, in.border_color, clamp(in_border / alpha, 0.0, 1.0));
        return float4(color.rgb, color.a * alpha);
    }

    return float4(in.background.rgb, in.background.a * alpha);
}
"#;

/// Metal shader for shadow rendering
pub const SHADOW_SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct ShadowInstance {
    float4 bounds;        // x, y, width, height
    float4 corner_radii;  // top_left, top_right, bottom_right, bottom_left
    float blur_radius;
    float4 color;
    float3 _padding;
};

struct Uniforms {
    float2 viewport_size;
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float4 color;
    float blur_radius;
    float2 size;
    float4 corner_radii;
};

float4 to_clip_space(float2 position, float2 viewport_size) {
    float2 ndc = (position / viewport_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    return float4(ndc, 0.0, 1.0);
}

vertex VertexOut shadow_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    constant ShadowInstance* instances [[buffer(0)]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    float2 positions[6] = {
        float2(0, 0), float2(1, 0), float2(0, 1),
        float2(1, 0), float2(1, 1), float2(0, 1)
    };

    ShadowInstance instance = instances[instance_id];
    float2 unit_pos = positions[vertex_id];

    // Expand bounds for blur
    float expand = instance.blur_radius * 2.0;
    float2 origin = instance.bounds.xy - expand;
    float2 size = instance.bounds.zw + expand * 2.0;
    float2 pixel_pos = origin + unit_pos * size;

    VertexOut out;
    out.position = to_clip_space(pixel_pos, uniforms.viewport_size);
    out.uv = unit_pos;
    out.color = instance.color;
    out.blur_radius = instance.blur_radius;
    out.size = instance.bounds.zw;
    out.corner_radii = instance.corner_radii;

    return out;
}

float rounded_rect_sdf(float2 point, float2 half_size, float radius) {
    float2 d = abs(point) - half_size + radius;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;
}

// Gaussian blur approximation
float gaussian(float x, float sigma) {
    return exp(-(x * x) / (2.0 * sigma * sigma));
}

fragment float4 shadow_fragment(VertexOut in [[stage_in]]) {
    float blur = in.blur_radius;
    float2 expand = blur * 2.0;
    float2 size = in.size;
    float2 expanded_size = size + expand * 2.0;

    // Map UV to original rect coordinates
    float2 point = in.uv * expanded_size - expand;
    float2 half_size = size / 2.0;
    float2 rel_point = point - half_size;

    float radius = in.corner_radii.x;  // Simplified: use top-left radius
    radius = min(radius, min(half_size.x, half_size.y));

    float dist = rounded_rect_sdf(rel_point, half_size, radius);

    // Soft shadow using gaussian falloff
    float sigma = blur / 2.0;
    float shadow_alpha = 1.0 - smoothstep(0.0, blur, dist);
    shadow_alpha *= gaussian(max(0.0, dist), sigma);

    return float4(in.color.rgb, in.color.a * shadow_alpha);
}
"#;
