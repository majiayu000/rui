//! WGSL ports of the Metal shaders in `renderer/metal/shaders.rs`. Each function
//! keeps the Metal one's name and arithmetic, so a change to one set is easy to
//! mirror in the other. Two things differ because WGSL requires them:
//! derivatives (`fwidth`) are taken before any branch on the sprite's kind, and
//! textures are sampled with `textureSampleLevel` at level 0, which is what
//! `sample` reads from the single-level atlas pages and images.

/// Color helpers placed ahead of every shader, as `COLOR_SHADER` is in Metal.
pub(super) const COLOR_SHADER: &str = r#"
fn srgb_to_linear(c: f32) -> f32 {
    return select(pow((c + 0.055) / 1.055, 2.4), c / 12.92, c <= 0.04045);
}

fn srgb_to_linear3(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
}

fn linear_color(srgb: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(srgb_to_linear3(saturate(srgb.rgb)), srgb.a);
}

// Oklab, from https://bottosson.github.io/posts/oklab/
fn linear_to_oklab(c: vec3<f32>) -> vec3<f32> {
    var lms = vec3<f32>(
        0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b,
        0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b,
        0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b);
    lms = sign(lms) * pow(abs(lms), vec3<f32>(1.0 / 3.0));
    return vec3<f32>(
        0.2104542553 * lms.x + 0.7936177850 * lms.y - 0.0040720468 * lms.z,
        1.9779984951 * lms.x - 2.4285922050 * lms.y + 0.4505937099 * lms.z,
        0.0259040371 * lms.x + 0.7827717662 * lms.y - 0.8086757660 * lms.z);
}

fn oklab_to_linear(lab: vec3<f32>) -> vec3<f32> {
    var lms = vec3<f32>(
        lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z,
        lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z,
        lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z);
    lms = lms * lms * lms;
    return vec3<f32>(
        4.0767416621 * lms.x - 3.3077115913 * lms.y + 0.2309699292 * lms.z,
        -1.2684380046 * lms.x + 2.6097574011 * lms.y - 0.3413193965 * lms.z,
        -0.0041960863 * lms.x - 0.7034186147 * lms.y + 1.7076147010 * lms.z);
}

// Convert from pixel coordinates to clip space
fn to_clip_space(position: vec2<f32>, viewport_size: vec2<f32>) -> vec4<f32> {
    var ndc = (position / viewport_size) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    return vec4<f32>(ndc, 0.0, 1.0);
}

// Signed distance function for rounded rectangle
fn rounded_rect_sdf(point: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let d = abs(point) - half_size + radius;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

// Pick corner radius based on position
fn pick_corner_radius(uv: vec2<f32>, radii: vec4<f32>) -> f32 {
    let left = select(radii.w, radii.x, uv.y < 0.5);   // top_left or bottom_left
    let right = select(radii.z, radii.y, uv.y < 0.5);  // top_right or bottom_right
    return select(right, left, uv.x < 0.5);
}

struct Uniforms {
    viewport_size: vec2<f32>,    // logical pixels
    drawable_size: vec2<f32>,    // device pixels
};

fn unit_position(vertex_id: u32) -> vec2<f32> {
    // Unit quad vertices (two triangles)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    return positions[vertex_id];
}
"#;

/// `SPRITE_SHADER`: quads, shadows, glyph sprites and paths in one instanced
/// pipeline, each instance a `SpriteRef` into its kind's buffer.
pub(super) const SPRITE_SHADER: &str = r#"
const KIND_QUAD: u32 = 0u;
const KIND_SHADOW: u32 = 1u;
const KIND_GLYPH: u32 = 2u;
const KIND_PATH: u32 = 3u;
const KIND_PATH_SPRITE: u32 = 4u;
const KIND_COLOR_GLYPH: u32 = 5u;
// `VertexOut::kind` holds the kind in its low bits and these flags above.
const KIND_MASK: u32 = 7u;
// The sprite crosses its clip, so its fragments are tested against it.
const CROSSES_CLIP: u32 = 8u;
// A quad with a solid fill, square corners and no border.
const PLAIN_QUAD: u32 = 16u;

const PI: f32 = 3.14159265358979323846;
const SQRT1_2: f32 = 0.70710678118654752440;

struct SpriteRef {
    index: u32,              // kind << 29 | index into that kind's buffer
    clip: u32,
};

struct QuadInstance {
    bounds: vec4<f32>,           // x, y, width, height
    background: vec4<f32>,
    border_color: vec4<f32>,
    border_widths: vec4<f32>,    // top, right, bottom, left
    corner_radii: vec4<f32>,     // top_left, top_right, bottom_right, bottom_left
    gradient_start: vec4<f32>,
    gradient_end: vec4<f32>,
    gradient_params: vec4<f32>,  // x=fill_type (0 solid, 1 linear, 2 radial), y=angle (radians)
};

struct ShadowInstance {
    bounds: vec4<f32>,
    corner_radii: vec4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,           // x = blur_radius
};

struct GlyphInstance {
    bounds: vec4<f32>,   // x, y, width, height in logical pixels
    uv: vec4<f32>,       // x, y, width, height in normalized atlas coordinates
    color: vec4<f32>,
};

struct PathInstance {
    bounds: vec4<f32>,
    color: vec4<f32>,
    edges: vec2<u32>,            // first edge and count in the edge buffer
    top: f32,                    // device y of the tile's first row
    _padding: f32,
    backdrop: array<f32, 16>,    // per row, the area edges right of the tile add
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> refs: array<SpriteRef>;
@group(0) @binding(2) var<storage, read> quads: array<QuadInstance>;
@group(0) @binding(3) var<storage, read> shadows: array<ShadowInstance>;
@group(0) @binding(4) var<storage, read> glyphs: array<GlyphInstance>;
@group(0) @binding(5) var<storage, read> clips: array<vec4<f32>>;
@group(0) @binding(6) var<storage, read> paths: array<PathInstance>;
@group(0) @binding(7) var<storage, read> edges: array<vec4<f32>>;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var color_atlas: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;

// Fields are shared between kinds where their meaning matches.
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) kind: u32,     // kind and flags
    @location(1) @interpolate(flat) clip: vec4<f32>, // x0, y0, x1, y1 in device pixels
    @location(2) uv: vec2<f32>,          // quad: unit position; glyph: atlas position
    @location(3) color: vec4<f32>,       // quad: background; shadow and glyph: color
    @location(4) border_color: vec4<f32>,
    @location(5) border_widths: vec4<f32>,
    @location(6) corner_radii: vec4<f32>,
    @location(7) gradient_start: vec4<f32>,
    @location(8) gradient_end: vec4<f32>,
    @location(9) gradient_params: vec4<f32>,
    @location(10) size: vec2<f32>,       // quad: size; shadow: half size
    @location(11) point: vec2<f32>,      // shadow: position relative to the rectangle's center
    @location(12) scalar: f32,           // shadow: sigma; glyph: luma
    @location(13) @interpolate(flat) path: u32,    // path: index of the tile
};

// The corner `unit_pos` of the rectangle at `origin` of `size`. Marks the sprite
// CROSSES_CLIP unless the rectangle lies inside `clip`.
fn corner(out: ptr<function, VertexOut>, origin: vec2<f32>, size: vec2<f32>,
          unit_pos: vec2<f32>, clip: vec4<f32>) -> vec2<f32> {
    let low = min(origin, origin + size);
    let high = max(origin, origin + size);
    if (any(low < clip.xy) || any(high > clip.zw)) {
        (*out).kind |= CROSSES_CLIP;
    }
    return origin + unit_pos * size;
}

fn quad_vertex(out: ptr<function, VertexOut>, instance: QuadInstance, unit_pos: vec2<f32>,
               clip: vec4<f32>, viewport_size: vec2<f32>) {
    let origin = instance.bounds.xy;
    let size = instance.bounds.zw;
    let pixel_pos = corner(out, origin, size, unit_pos, clip);

    (*out).position = to_clip_space(pixel_pos, viewport_size);
    (*out).uv = unit_pos;
    (*out).color = linear_color(instance.background);
    if (all(instance.corner_radii == vec4<f32>(0.0)) && all(instance.border_widths == vec4<f32>(0.0)) &&
        instance.gradient_params.x < 0.5) {
        (*out).kind |= PLAIN_QUAD;
    }
    (*out).border_color = linear_color(instance.border_color);
    (*out).border_widths = instance.border_widths;
    (*out).corner_radii = instance.corner_radii;
    // Gradient stops go to the fragment shader as premultiplied oklab.
    let start = linear_color(instance.gradient_start);
    let end = linear_color(instance.gradient_end);
    (*out).gradient_start = vec4<f32>(linear_to_oklab(start.rgb) * start.a, start.a);
    (*out).gradient_end = vec4<f32>(linear_to_oklab(end.rgb) * end.a, end.a);
    (*out).gradient_params = instance.gradient_params;
    (*out).size = size;
}

fn shadow_vertex(out: ptr<function, VertexOut>, instance: ShadowInstance, unit_pos: vec2<f32>,
                 clip: vec4<f32>, viewport_size: vec2<f32>) {
    // Past three sigma the blur adds nothing visible. One more pixel keeps the
    // antialiased edge of an unblurred shadow.
    let sigma = max(instance.params.x, 0.0) * 0.5;
    let margin = 3.0 * sigma + 1.0;
    let half_size = instance.bounds.zw * 0.5;
    let center = instance.bounds.xy + half_size;
    let pixel_pos = corner(out, center - half_size - margin,
                           instance.bounds.zw + 2.0 * margin, unit_pos, clip);

    (*out).position = to_clip_space(pixel_pos, viewport_size);
    (*out).point = pixel_pos - center;
    (*out).size = half_size;
    (*out).color = linear_color(instance.color);
    (*out).corner_radii = instance.corner_radii;
    (*out).scalar = sigma;
}

fn glyph_vertex(out: ptr<function, VertexOut>, instance: GlyphInstance, unit_pos: vec2<f32>,
                clip: vec4<f32>, viewport_size: vec2<f32>) {
    (*out).position = to_clip_space(
        corner(out, instance.bounds.xy, instance.bounds.zw, unit_pos, clip), viewport_size);
    (*out).uv = instance.uv.xy + unit_pos * instance.uv.zw;
    (*out).color = linear_color(instance.color);
    (*out).scalar = dot(saturate(instance.color.rgb), vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn path_vertex(out: ptr<function, VertexOut>, index: u32, unit_pos: vec2<f32>,
               clip: vec4<f32>, viewport_size: vec2<f32>) {
    let bounds = paths[index].bounds;
    (*out).position = to_clip_space(
        corner(out, bounds.xy, bounds.zw, unit_pos, clip), viewport_size);
    (*out).color = linear_color(paths[index].color);
    (*out).path = index;
}

@vertex
fn sprite_vertex(@builtin(vertex_index) vertex_id: u32,
                 @builtin(instance_index) instance_id: u32) -> VertexOut {
    let sprite = refs[instance_id];
    let kind = sprite.index >> 29u;
    let index = sprite.index & 0x1fffffffu;
    let unit_pos = unit_position(vertex_id);

    var out: VertexOut;
    out.kind = kind;
    out.clip = clips[sprite.clip];
    let viewport_size = uniforms.viewport_size;
    let clip = out.clip * (viewport_size / uniforms.drawable_size).xyxy;
    if (kind == KIND_QUAD) {
        quad_vertex(&out, quads[index], unit_pos, clip, viewport_size);
    } else if (kind == KIND_SHADOW) {
        shadow_vertex(&out, shadows[index], unit_pos, clip, viewport_size);
    } else if (kind == KIND_PATH) {
        path_vertex(&out, index, unit_pos, clip, viewport_size);
    } else {
        glyph_vertex(&out, glyphs[index], unit_pos, clip, viewport_size);
    }
    return out;
}

// Mix premultiplied oklab stops and return a linear color.
fn mix_oklab(start: vec4<f32>, end: vec4<f32>, t: f32) -> vec4<f32> {
    let lab = mix(start, end, t);
    if (lab.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(saturate(oklab_to_linear(lab.xyz / lab.a)), lab.a);
}

fn resolve_fill(in: VertexOut) -> vec4<f32> {
    let fill_type = in.gradient_params.x;
    if (fill_type < 0.5) {
        return in.color;
    } else if (fill_type < 1.5) {
        let angle = in.gradient_params.y;
        let dir = vec2<f32>(cos(angle), sin(angle));
        let uv = in.uv - vec2<f32>(0.5, 0.5);
        let t = clamp(dot(uv, dir) + 0.5, 0.0, 1.0);
        return mix_oklab(in.gradient_start, in.gradient_end, t);
    } else {
        let uv = in.uv - vec2<f32>(0.5, 0.5);
        let t = clamp(length(uv) * 2.0, 0.0, 1.0);
        return mix_oklab(in.gradient_start, in.gradient_end, t);
    }
}

// The quad's outer distance: `plain_quad_fragment` and `quad_fragment` in Metal
// compute the same value (a plain quad's radii are all zero).
fn quad_distance(in: VertexOut) -> f32 {
    let half_size = in.size / 2.0;
    let rel_point = in.uv * in.size - half_size;
    let corner_radius = min(pick_corner_radius(in.uv, in.corner_radii), min(half_size.x, half_size.y));
    return rounded_rect_sdf(rel_point, half_size, corner_radius);
}

// `plain_quad_fragment` for a PLAIN_QUAD.
fn plain_quad_fragment(in: VertexOut, dist: f32, aa: f32) -> vec4<f32> {
    let alpha = 1.0 - smoothstep(-aa, aa, dist);
    if (alpha < 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

fn quad_fragment(in: VertexOut, dist: f32, aa: f32) -> vec4<f32> {
    let half_size = in.size / 2.0;
    let rel_point = in.uv * in.size - half_size;
    let corner_radius = min(pick_corner_radius(in.uv, in.corner_radii), min(half_size.x, half_size.y));
    let alpha = 1.0 - smoothstep(-aa, aa, dist);

    if (alpha < 0.001) {
        discard;
    }

    let fill_color = resolve_fill(in);

    // Border handling
    let border_width = max(max(in.border_widths.x, in.border_widths.y),
                           max(in.border_widths.z, in.border_widths.w));

    if (border_width > 0.0) {
        // Inner edge for border
        let inner_half_size = half_size - border_width;
        let inner_radius = max(0.0, corner_radius - border_width);
        let inner_dist = rounded_rect_sdf(rel_point, inner_half_size, inner_radius);

        let border_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);
        let in_border = alpha - border_alpha;

        // Mix border and background
        let color = mix(fill_color, in.border_color, clamp(in_border / alpha, 0.0, 1.0));
        return vec4<f32>(color.rgb, color.a * alpha);
    }

    return vec4<f32>(fill_color.rgb, fill_color.a * alpha);
}

// Abramowitz and Stegun 7.1.27, accurate to 5e-4.
fn erf2(x: vec2<f32>) -> vec2<f32> {
    let s = sign(x);
    let a = abs(x);
    var r = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    r *= r;
    return s - s / (r * r);
}

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * PI) * sigma);
}

// The blurred step from -edge to edge, at x.
fn blur_span(x: f32, edge: f32, sigma: f32) -> f32 {
    let integral = 0.5 + 0.5 * erf2((x + vec2<f32>(-edge, edge)) * (SQRT1_2 / sigma));
    return integral.y - integral.x;
}

// The row of the rounded rectangle at height y, blurred along x.
fn blur_along_x(x: f32, y: f32, sigma: f32, corner: f32, half_size: vec2<f32>) -> f32 {
    let delta = min(half_size.y - corner - abs(y), 0.0);
    let curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    return blur_span(x, curved, sigma);
}

fn shadow_fragment(in: VertexOut) -> vec4<f32> {
    let point = in.point;
    let half_size = in.size;
    let sigma = in.scalar;

    var corner = select(
        select(in.corner_radii.z, in.corner_radii.y, point.y < 0.0),
        select(in.corner_radii.w, in.corner_radii.x, point.y < 0.0),
        point.x < 0.0);
    corner = clamp(corner, 0.0, min(half_size.x, half_size.y));

    var alpha: f32;
    if (sigma <= 0.0) {
        alpha = saturate(0.5 - rounded_rect_sdf(point, half_size, corner));
    } else if (corner <= 0.0) {
        alpha = blur_span(point.x, half_size.x, sigma) * blur_span(point.y, half_size.y, sigma);
    } else {
        // Sum the blurred rows within three sigma, weighted by the Gaussian along y.
        let low = point.y - half_size.y;
        let high = point.y + half_size.y;
        let start = clamp(-3.0 * sigma, low, high);
        let end = clamp(3.0 * sigma, low, high);
        let step = (end - start) / 8.0;
        var y = start + step * 0.5;
        alpha = 0.0;
        for (var i = 0; i < 8; i++) {
            alpha += blur_along_x(point.x, point.y - y, sigma, corner, half_size)
                * gaussian(y, sigma) * step;
            y += step;
        }
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

fn glyph_fragment(in: VertexOut) -> vec4<f32> {
    let coverage = textureSampleLevel(atlas, samp, in.uv, 0.0).r;
    // Raw coverage blended in linear light thins dark text and bolds light text.
    // Remap it so text keeps the ink it had when blending encoded values.
    let dark = 1.0 - srgb_to_linear(1.0 - coverage);
    let light = srgb_to_linear(coverage);
    return vec4<f32>(in.color.rgb, in.color.a * mix(dark, light, in.scalar));
}

// The mean of clamp(u, 0, 1) as u runs linearly from u0 to u1.
fn mean_clamped(u0: f32, u1: f32) -> f32 {
    if (abs(u1 - u0) < 1e-4) {
        return saturate((u0 + u1) * 0.5);
    }
    // An antiderivative of clamp(u, 0, 1).
    let u = vec2<f32>(u0, u1);
    let integral = select(select(u - 0.5, u * u * 0.5, u <= vec2<f32>(1.0)), vec2<f32>(0.0),
                          u <= vec2<f32>(0.0));
    return (integral.y - integral.x) / (u1 - u0);
}

// The share of the pixel square the path winds around, nonzero rule;
// `coverage` in `plan/path.rs` on the CPU.
fn path_coverage(in: VertexOut) -> f32 {
    let left = in.position.x - 0.5;
    let top = in.position.y - 0.5;
    let bottom = in.position.y + 0.5;
    var area = paths[in.path].backdrop[clamp(i32(top - paths[in.path].top), 0, 15)];
    let first = paths[in.path].edges.x;
    let end = first + paths[in.path].edges.y;
    for (var i = first; i < end; i++) {
        let edge = edges[i];
        if (edge.y == edge.w) {
            continue;
        }
        // Downward edges add, upward ones subtract.
        let sign = select(-1.0, 1.0, edge.y < edge.w);
        let e = select(edge.zwxy, edge, edge.y < edge.w);
        let low = max(e.y, top);
        let high = min(e.w, bottom);
        if (low >= high) {
            continue;
        }
        let slope = (e.z - e.x) / (e.w - e.y);
        let u0 = e.x + (low - e.y) * slope - left;
        let u1 = e.x + (high - e.y) * slope - left;
        area += sign * (high - low) * mean_clamped(u0, u1);
    }
    return min(abs(area), 1.0);
}

@fragment
fn sprite_fragment(in: VertexOut) -> @location(0) vec4<f32> {
    // Derivatives first, while every fragment of the quad runs them.
    let dist = quad_distance(in);
    let aa = fwidth(dist);

    var color: vec4<f32>;
    let kind = in.kind & KIND_MASK;
    if ((in.kind & PLAIN_QUAD) != 0u) {
        color = plain_quad_fragment(in, dist, aa);
    } else if (kind == KIND_QUAD) {
        color = quad_fragment(in, dist, aa);
    } else if (kind == KIND_SHADOW) {
        color = shadow_fragment(in);
    } else if (kind == KIND_GLYPH) {
        color = glyph_fragment(in);
    } else if (kind == KIND_PATH) {
        color = vec4<f32>(in.color.rgb, in.color.a * path_coverage(in));
    } else if (kind == KIND_PATH_SPRITE) {
        color = vec4<f32>(in.color.rgb, in.color.a * textureSampleLevel(atlas, samp, in.uv, 0.0).r);
    } else {
        // A color glyph keeps its own colors, read as linear from the sRGB
        // page, and takes only the text's opacity.
        let texel = textureSampleLevel(color_atlas, samp, in.uv, 0.0);
        color = vec4<f32>(texel.rgb, texel.a * in.color.a);
    }
    // Pixel centers sit on half pixels, so this keeps exactly the pixels a
    // scissor rectangle of the same integer edges would.
    let pixel = in.position.xy;
    if ((in.kind & CROSSES_CLIP) != 0u &&
        (pixel.x < in.clip.x || pixel.y < in.clip.y || pixel.x >= in.clip.z || pixel.y >= in.clip.w)) {
        discard;
    }
    return color;
}
"#;

/// `IMAGE_SHADER`: one image per draw, its instance picked by the draw's
/// instance index from the frame's image array.
pub(super) const IMAGE_SHADER: &str = r#"
struct ImageInstance {
    bounds: vec4<f32>,
    corner_radii: vec4<f32>,
    color: vec4<f32>,    // multiply color
    params: vec4<f32>,   // x = opacity
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> images: array<ImageInstance>;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) corner_radii: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) opacity: f32,
};

@vertex
fn image_vertex(@builtin(vertex_index) vertex_id: u32,
                @builtin(instance_index) instance_id: u32) -> VertexOut {
    let instance = images[instance_id];
    let unit_pos = unit_position(vertex_id);
    let origin = instance.bounds.xy;
    let size = instance.bounds.zw;
    let pixel_pos = origin + unit_pos * size;

    var out: VertexOut;
    out.position = to_clip_space(pixel_pos, uniforms.viewport_size);
    out.uv = unit_pos;
    out.size = size;
    out.corner_radii = instance.corner_radii;
    out.color = linear_color(instance.color);
    out.opacity = instance.params.x;
    return out;
}

@fragment
fn image_fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let size = in.size;
    let half_size = size / 2.0;
    let rel_point = in.uv * size - half_size;

    let corner_radius = min(pick_corner_radius(in.uv, in.corner_radii), min(half_size.x, half_size.y));
    let dist = rounded_rect_sdf(rel_point, half_size, corner_radius);
    let aa = fwidth(dist);
    let alpha = 1.0 - smoothstep(-aa, aa, dist);

    if (alpha < 0.001) {
        discard;
    }

    let tex_color = textureSampleLevel(tex, samp, in.uv, 0.0);
    let color = vec4<f32>(tex_color.rgb * in.color.rgb, tex_color.a);
    let out_alpha = color.a * in.color.a * in.opacity * alpha;
    return vec4<f32>(color.rgb, out_alpha);
}
"#;

/// `RETAINED_SHADER`: the full-screen copy and the damage clear.
pub(super) const RETAINED_SHADER: &str = r#"
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> clear_color: vec4<f32>;

@vertex
fn fullscreen_vertex(@builtin(vertex_index) vertex_id: u32) -> @builtin(position) vec4<f32> {
    // One triangle over the whole target.
    let corner = vec2<f32>(f32((vertex_id << 1u) & 2u), f32(vertex_id & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn copy_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(position.xy), 0);
}

@fragment
fn clear_fragment() -> @location(0) vec4<f32> {
    return clear_color;
}
"#;
