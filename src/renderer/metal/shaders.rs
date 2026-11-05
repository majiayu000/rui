//! Metal shader source code

/// Color helpers compiled ahead of every shader. Scene colors are sRGB encoded;
/// shaders convert them to linear light, and the sRGB render target encodes the
/// blended result again.
pub const COLOR_SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

float srgb_to_linear(float c) {
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

float3 srgb_to_linear(float3 c) {
    return float3(srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
}

float4 linear_color(float4 srgb) {
    return float4(srgb_to_linear(saturate(srgb.rgb)), srgb.a);
}

// Oklab, from https://bottosson.github.io/posts/oklab/
float3 linear_to_oklab(float3 c) {
    float3 lms = float3(
        0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b,
        0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b,
        0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b);
    lms = sign(lms) * pow(abs(lms), 1.0 / 3.0);
    return float3(
        0.2104542553 * lms.x + 0.7936177850 * lms.y - 0.0040720468 * lms.z,
        1.9779984951 * lms.x - 2.4285922050 * lms.y + 0.4505937099 * lms.z,
        0.0259040371 * lms.x + 0.7827717662 * lms.y - 0.8086757660 * lms.z);
}

float3 oklab_to_linear(float3 lab) {
    float3 lms = float3(
        lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z,
        lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z,
        lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z);
    lms = lms * lms * lms;
    return float3(
        4.0767416621 * lms.x - 3.3077115913 * lms.y + 0.2309699292 * lms.z,
        -1.2684380046 * lms.x + 2.6097574011 * lms.y - 0.3413193965 * lms.z,
        -0.0041960863 * lms.x - 0.7034186147 * lms.y + 1.7076147010 * lms.z);
}
"#;

/// One instanced pipeline for quads, shadows, glyph sprites and paths. Each
/// instance is a reference: the top three bits of `index` pick the kind and the
/// rest index that kind's buffer, so draws of different kinds share a call while each kind keeps
/// its own compact layout. `clip` indexes device pixel clip rectangles, applied
/// per fragment as a scissor rectangle would be.
///
/// Quads are rounded rectangles with borders and solid or oklab gradient fills.
/// Shadows are a rounded rectangle blurred by a Gaussian with sigma =
/// blur_radius / 2, as CSS box shadows are: separable for square corners and
/// integrated numerically along y for rounded ones (Evan Wallace, "Fast Rounded
/// Rectangle Shadows"; GPUI draws its shadows the same way). Glyphs sample an A8
/// atlas page. Paths are tiles whose fragments take the exact area of the pixel
/// square the path winds around, from the edges the tile lists and the backdrop
/// of the edges right of it (see `path.rs`, whose `coverage` is the same
/// function), or, once rasterized into the atlas, sprites whose coverage is
/// used as is.
pub const SPRITE_SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

constant uint KIND_QUAD = 0;
constant uint KIND_SHADOW = 1;
constant uint KIND_GLYPH = 2;
constant uint KIND_PATH = 3;
constant uint KIND_PATH_SPRITE = 4;

struct SpriteRef {
    uint index;              // kind << 29 | index into that kind's buffer
    uint clip;
};

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

struct ShadowInstance {
    float4 bounds;        // x, y, width, height
    float4 corner_radii;  // top_left, top_right, bottom_right, bottom_left
    float4 color;         // r, g, b, a
    float4 params;        // x = blur_radius
};

struct GlyphInstance {
    float4 bounds;    // x, y, width, height in logical pixels
    float4 uv;        // x, y, width, height in normalized atlas coordinates
    float4 color;
};

struct PathInstance {
    float4 bounds;        // x, y, width, height in logical pixels
    float4 color;
    uint2 edges;          // first edge and count in the edge buffer
    float top;            // device y of the tile's first row
    float _padding;
    float backdrop[16];   // per row, the area edges right of the tile add
};

struct Uniforms {
    float2 viewport_size;
};

// Fields are shared between kinds where their meaning matches.
struct VertexOut {
    float4 position [[position]];
    uint kind [[flat]];
    float4 clip [[flat]];    // x0, y0, x1, y1 in device pixels
    float2 uv;               // quad: unit position; glyph: atlas position
    float4 color;            // quad: background; shadow and glyph: color
    float4 border_color;
    float4 border_widths;
    float4 corner_radii;     // quad and shadow
    float4 gradient_start;
    float4 gradient_end;
    float4 gradient_params;
    float2 size;             // quad: size; shadow: half size
    float2 point;            // shadow: position relative to the rectangle's center
    float scalar;            // shadow: sigma; glyph: luma
    uint path [[flat]];      // path: index of the tile
};

// Convert from pixel coordinates to clip space
float4 to_clip_space(float2 position, float2 viewport_size) {
    float2 ndc = (position / viewport_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;  // Flip Y for Metal coordinate system
    return float4(ndc, 0.0, 1.0);
}

void quad_vertex(thread VertexOut& out, QuadInstance instance, float2 unit_pos,
                 float2 viewport_size) {
    // Calculate pixel position
    float2 origin = instance.bounds.xy;
    float2 size = instance.bounds.zw;
    float2 pixel_pos = origin + unit_pos * size;

    out.position = to_clip_space(pixel_pos, viewport_size);
    out.uv = unit_pos;
    out.color = linear_color(instance.background);
    out.border_color = linear_color(instance.border_color);
    out.border_widths = instance.border_widths;
    out.corner_radii = instance.corner_radii;
    // Gradient stops go to the fragment shader as premultiplied oklab.
    float4 start = linear_color(instance.gradient_start);
    float4 end = linear_color(instance.gradient_end);
    out.gradient_start = float4(linear_to_oklab(start.rgb) * start.a, start.a);
    out.gradient_end = float4(linear_to_oklab(end.rgb) * end.a, end.a);
    out.gradient_params = instance.gradient_params;
    out.size = size;
}

void shadow_vertex(thread VertexOut& out, ShadowInstance instance, float2 unit_pos,
                   float2 viewport_size) {
    // Past three sigma the blur adds nothing visible. One more pixel keeps the
    // antialiased edge of an unblurred shadow.
    float sigma = max(instance.params.x, 0.0) * 0.5;
    float margin = 3.0 * sigma + 1.0;
    float2 half_size = instance.bounds.zw * 0.5;
    float2 center = instance.bounds.xy + half_size;
    float2 pixel_pos = center - half_size - margin + unit_pos * (instance.bounds.zw + 2.0 * margin);

    out.position = to_clip_space(pixel_pos, viewport_size);
    out.point = pixel_pos - center;
    out.size = half_size;
    out.color = linear_color(instance.color);
    out.corner_radii = instance.corner_radii;
    out.scalar = sigma;
}

void glyph_vertex(thread VertexOut& out, GlyphInstance instance, float2 unit_pos,
                  float2 viewport_size) {
    out.position = to_clip_space(instance.bounds.xy + unit_pos * instance.bounds.zw,
                                 viewport_size);
    out.uv = instance.uv.xy + unit_pos * instance.uv.zw;
    out.color = linear_color(instance.color);
    out.scalar = dot(saturate(instance.color.rgb), float3(0.2126, 0.7152, 0.0722));
}

void path_vertex(thread VertexOut& out, constant PathInstance& instance, uint index,
                 float2 unit_pos, float2 viewport_size) {
    out.position = to_clip_space(instance.bounds.xy + unit_pos * instance.bounds.zw,
                                 viewport_size);
    out.color = linear_color(instance.color);
    out.path = index;
}

vertex VertexOut sprite_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    constant SpriteRef* refs [[buffer(0)]],
    constant Uniforms& uniforms [[buffer(1)]],
    constant QuadInstance* quads [[buffer(2)]],
    constant ShadowInstance* shadows [[buffer(3)]],
    constant GlyphInstance* glyphs [[buffer(4)]],
    constant float4* clips [[buffer(5)]],
    constant PathInstance* paths [[buffer(6)]]
) {
    // Unit quad vertices (two triangles)
    float2 positions[6] = {
        float2(0, 0), float2(1, 0), float2(0, 1),
        float2(1, 0), float2(1, 1), float2(0, 1)
    };

    SpriteRef sprite = refs[instance_id];
    uint kind = sprite.index >> 29;
    uint index = sprite.index & 0x1fffffff;
    float2 unit_pos = positions[vertex_id];

    VertexOut out = {};
    out.kind = kind;
    out.clip = clips[sprite.clip];
    if (kind == KIND_QUAD) {
        quad_vertex(out, quads[index], unit_pos, uniforms.viewport_size);
    } else if (kind == KIND_SHADOW) {
        shadow_vertex(out, shadows[index], unit_pos, uniforms.viewport_size);
    } else if (kind == KIND_PATH) {
        path_vertex(out, paths[index], index, unit_pos, uniforms.viewport_size);
    } else {
        glyph_vertex(out, glyphs[index], unit_pos, uniforms.viewport_size);
    }
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

// Mix premultiplied oklab stops and return a linear color.
float4 mix_oklab(float4 start, float4 end, float t) {
    float4 lab = mix(start, end, t);
    if (lab.a <= 0.0) {
        return float4(0.0);
    }
    return float4(saturate(oklab_to_linear(lab.xyz / lab.a)), lab.a);
}

float4 resolve_fill(VertexOut in) {
    float fill_type = in.gradient_params.x;
    if (fill_type < 0.5) {
        return in.color;
    } else if (fill_type < 1.5) {
        float angle = in.gradient_params.y;
        float2 dir = float2(cos(angle), sin(angle));
        float2 uv = in.uv - float2(0.5, 0.5);
        float t = dot(uv, dir) + 0.5;
        t = clamp(t, 0.0, 1.0);
        return mix_oklab(in.gradient_start, in.gradient_end, t);
    } else {
        float2 uv = in.uv - float2(0.5, 0.5);
        float t = length(uv) * 2.0;
        t = clamp(t, 0.0, 1.0);
        return mix_oklab(in.gradient_start, in.gradient_end, t);
    }
}

float4 quad_fragment(VertexOut in) {
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

// Abramowitz and Stegun 7.1.27, accurate to 5e-4.
float2 erf(float2 x) {
    float2 s = sign(x);
    float2 a = abs(x);
    float2 r = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    r *= r;
    return s - s / (r * r);
}

float gaussian(float x, float sigma) {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * M_PI_F) * sigma);
}

// The blurred step from -edge to edge, at x.
float blur_span(float x, float edge, float sigma) {
    float2 integral = 0.5 + 0.5 * erf((x + float2(-edge, edge)) * (M_SQRT1_2_F / sigma));
    return integral.y - integral.x;
}

// The row of the rounded rectangle at height y, blurred along x.
float blur_along_x(float x, float y, float sigma, float corner, float2 half_size) {
    float delta = min(half_size.y - corner - abs(y), 0.0);
    float curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    return blur_span(x, curved, sigma);
}

float4 shadow_fragment(VertexOut in) {
    float2 point = in.point;
    float2 half_size = in.size;
    float sigma = in.scalar;

    float corner = point.x < 0.0
        ? (point.y < 0.0 ? in.corner_radii.x : in.corner_radii.w)
        : (point.y < 0.0 ? in.corner_radii.y : in.corner_radii.z);
    corner = clamp(corner, 0.0, min(half_size.x, half_size.y));

    float alpha;
    if (sigma <= 0.0) {
        alpha = saturate(0.5 - rounded_rect_sdf(point, half_size, corner));
    } else if (corner <= 0.0) {
        alpha = blur_span(point.x, half_size.x, sigma) * blur_span(point.y, half_size.y, sigma);
    } else {
        // Sum the blurred rows within three sigma, weighted by the Gaussian along y.
        float low = point.y - half_size.y;
        float high = point.y + half_size.y;
        float start = clamp(-3.0 * sigma, low, high);
        float end = clamp(3.0 * sigma, low, high);
        float step = (end - start) / 8.0;
        float y = start + step * 0.5;
        alpha = 0.0;
        for (int i = 0; i < 8; i++) {
            alpha += blur_along_x(point.x, point.y - y, sigma, corner, half_size)
                * gaussian(y, sigma) * step;
            y += step;
        }
    }

    return float4(in.color.rgb, in.color.a * alpha);
}

float4 glyph_fragment(VertexOut in, texture2d<float> atlas, sampler samp) {
    float coverage = atlas.sample(samp, in.uv).r;
    // Raw coverage blended in linear light thins dark text and bolds light text.
    // Remap it so text keeps the ink it had when blending encoded values: exact
    // for dark text on a light background and light on dark, mixed by luma between.
    float dark = 1.0 - srgb_to_linear(1.0 - coverage);
    float light = srgb_to_linear(coverage);
    return float4(in.color.rgb, in.color.a * mix(dark, light, in.scalar));
}

// The mean of clamp(u, 0, 1) as u runs linearly from u0 to u1.
float mean_clamped(float u0, float u1) {
    if (abs(u1 - u0) < 1e-4) {
        return saturate((u0 + u1) * 0.5);
    }
    // An antiderivative of clamp(u, 0, 1).
    float2 u = float2(u0, u1);
    float2 integral = select(select(u - 0.5, u * u * 0.5, u <= 1.0), float2(0.0), u <= 0.0);
    return (integral.y - integral.x) / (u1 - u0);
}

// The share of the pixel square the path winds around, nonzero rule;
// `coverage` in `path.rs` on the CPU.
float path_coverage(VertexOut in, constant float4* edges, constant PathInstance* paths) {
    constant PathInstance& tile = paths[in.path];
    float left = in.position.x - 0.5;
    float top = in.position.y - 0.5;
    float bottom = in.position.y + 0.5;
    float area = tile.backdrop[clamp(int(top - tile.top), 0, 15)];
    uint end = tile.edges.x + tile.edges.y;
    for (uint i = tile.edges.x; i < end; i++) {
        float4 edge = edges[i];
        if (edge.y == edge.w) {
            continue;
        }
        // Downward edges add, upward ones subtract.
        float sign = edge.y < edge.w ? 1.0 : -1.0;
        float4 e = edge.y < edge.w ? edge : edge.zwxy;
        float low = max(e.y, top);
        float high = min(e.w, bottom);
        if (low >= high) {
            continue;
        }
        float slope = (e.z - e.x) / (e.w - e.y);
        float u0 = e.x + (low - e.y) * slope - left;
        float u1 = e.x + (high - e.y) * slope - left;
        area += sign * (high - low) * mean_clamped(u0, u1);
    }
    return min(abs(area), 1.0);
}

fragment float4 sprite_fragment(VertexOut in [[stage_in]],
                                texture2d<float> atlas [[texture(0)]],
                                sampler samp [[sampler(0)]],
                                constant float4* edges [[buffer(0)]],
                                constant PathInstance* paths [[buffer(1)]]) {
    float4 color;
    if (in.kind == KIND_QUAD) {
        color = quad_fragment(in);
    } else if (in.kind == KIND_SHADOW) {
        color = shadow_fragment(in);
    } else if (in.kind == KIND_GLYPH) {
        color = glyph_fragment(in, atlas, samp);
    } else if (in.kind == KIND_PATH) {
        color = float4(in.color.rgb, in.color.a * path_coverage(in, edges, paths));
    } else {
        color = float4(in.color.rgb, in.color.a * atlas.sample(samp, in.uv).r);
    }
    // Pixel centers sit on half pixels, so this keeps exactly the pixels a
    // scissor rectangle of the same integer edges would.
    float2 pixel = in.position.xy;
    if (pixel.x < in.clip.x || pixel.y < in.clip.y || pixel.x >= in.clip.z || pixel.y >= in.clip.w) {
        discard_fragment();
    }
    return color;
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
    out.color = linear_color(instance.color);
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
