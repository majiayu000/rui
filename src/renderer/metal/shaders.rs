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
    float4 gradient_start;   // gradient start color
    float4 gradient_end;     // gradient end color
    float4 gradient_params;  // x=fill_type (0 solid, 1 linear, 2 radial), y=angle (radians)
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
    float4 gradient_start;
    float4 gradient_end;
    float4 gradient_params;
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
    out.gradient_start = instance.gradient_start;
    out.gradient_end = instance.gradient_end;
    out.gradient_params = instance.gradient_params;
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

float4 resolve_fill(VertexOut in) {
    float fill_type = in.gradient_params.x;
    if (fill_type < 0.5) {
        return in.background;
    } else if (fill_type < 1.5) {
        float angle = in.gradient_params.y;
        float2 dir = float2(cos(angle), sin(angle));
        float2 uv = in.uv - float2(0.5, 0.5);
        float t = dot(uv, dir) + 0.5;
        t = clamp(t, 0.0, 1.0);
        return mix(in.gradient_start, in.gradient_end, t);
    } else {
        float2 uv = in.uv - float2(0.5, 0.5);
        float t = length(uv) * 2.0;
        t = clamp(t, 0.0, 1.0);
        return mix(in.gradient_start, in.gradient_end, t);
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

    float4 fill_color = resolve_fill(in);

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
        float4 color = mix(fill_color, in.border_color, clamp(in_border / alpha, 0.0, 1.0));
        return float4(color.rgb, color.a * alpha);
    }

    return float4(fill_color.rgb, fill_color.a * alpha);
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

/// Metal shader for image/text rendering
pub const IMAGE_SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct ImageInstance {
    float4 bounds;        // x, y, width, height
    float4 corner_radii;  // top_left, top_right, bottom_right, bottom_left
    float4 color;         // multiply color
    float opacity;
    float3 _padding;
};

struct Uniforms {
    float2 viewport_size;
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float2 size;
    float4 corner_radii;
    float4 color;
    float opacity;
};

float4 to_clip_space(float2 position, float2 viewport_size) {
    float2 ndc = (position / viewport_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    return float4(ndc, 0.0, 1.0);
}

vertex VertexOut image_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    constant ImageInstance* instances [[buffer(0)]],
    constant Uniforms& uniforms [[buffer(1)]]
) {
    float2 positions[6] = {
        float2(0, 0), float2(1, 0), float2(0, 1),
        float2(1, 0), float2(1, 1), float2(0, 1)
    };

    ImageInstance instance = instances[instance_id];
    float2 unit_pos = positions[vertex_id];

    float2 origin = instance.bounds.xy;
    float2 size = instance.bounds.zw;
    float2 pixel_pos = origin + unit_pos * size;

    VertexOut out;
    out.position = to_clip_space(pixel_pos, uniforms.viewport_size);
    out.uv = unit_pos;
    out.size = size;
    out.corner_radii = instance.corner_radii;
    out.color = instance.color;
    out.opacity = instance.opacity;
    return out;
}

float rounded_rect_sdf(float2 point, float2 half_size, float radius) {
    float2 d = abs(point) - half_size + radius;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - radius;
}

float pick_corner_radius(float2 uv, float4 radii) {
    if (uv.x < 0.5) {
        return uv.y < 0.5 ? radii.x : radii.w;
    } else {
        return uv.y < 0.5 ? radii.y : radii.z;
    }
}

fragment float4 image_fragment(VertexOut in [[stage_in]],
                               texture2d<float> tex [[texture(0)]],
                               sampler samp [[sampler(0)]]) {
    float2 size = in.size;
    float2 half_size = size / 2.0;
    float2 center = half_size;
    float2 point = in.uv * size;
    float2 rel_point = point - center;

    float corner_radius = pick_corner_radius(in.uv, in.corner_radii);
    corner_radius = min(corner_radius, min(half_size.x, half_size.y));

    float dist = rounded_rect_sdf(rel_point, half_size, corner_radius);
    float aa = fwidth(dist);
    float alpha = 1.0 - smoothstep(-aa, aa, dist);

    if (alpha < 0.001) {
        discard_fragment();
    }

    float4 tex_color = tex.sample(samp, in.uv);
    float4 color = float4(tex_color.rgb * in.color.rgb, tex_color.a);
    float out_alpha = color.a * in.color.a * in.opacity * alpha;

    return float4(color.rgb, out_alpha);
}
"#;
