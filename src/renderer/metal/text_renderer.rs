//! Text rasterization using CoreText + CoreGraphics.
//! Produces a BGRA8 pixel buffer that can be uploaded as a Metal texture.

use crate::renderer::scene::TextItem;
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_graphics::color_space::CGColorSpace;
use core_graphics::context::CGContext;
use core_text::font as ct_font;
use core_text::line::CTLine;

/// Rasterize all text items into a BGRA8 pixel buffer.
pub fn rasterize_text_items(
    items: &[TextItem],
    viewport_width: u32,
    viewport_height: u32,
) -> Vec<u8> {
    let width = viewport_width as usize;
    let height = viewport_height as usize;

    if items.is_empty() || width == 0 || height == 0 {
        return vec![0u8; width * height * 4];
    }

    let bytes_per_row = width * 4;

    // kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little = BGRA premultiplied
    let bitmap_info: u32 = 2 | (2 << 12);
    let color_space = CGColorSpace::create_device_rgb();

    let mut context = CGContext::create_bitmap_context(
        None,
        width,
        height,
        8,
        bytes_per_row,
        &color_space,
        bitmap_info,
    );

    // 不做全局 Y 翻转，而是在每个文字项中手动转换坐标
    // CoreGraphics 使用左下角原点，UI 使用左上角原点

    for item in items {
        draw_text_item(&context, item, height as f64);
    }

    let data = context.data();
    let len = width * height * 4;
    unsafe { std::slice::from_raw_parts(data.as_ptr(), len) }.to_vec()
}

fn draw_text_item(context: &CGContext, item: &TextItem, viewport_height: f64) {
    if item.content.is_empty() {
        return;
    }

    let x = item.bounds.x() as f64;
    let y_ui = item.bounds.y() as f64;
    let max_width = item.bounds.width() as f64;
    let line_height = item.bounds.height() as f64;
    let font_size = item.font_size as f64;

    // 创建字体
    let font_name = item.font_family.as_deref().unwrap_or("Menlo");
    let ct_font = ct_font::new_from_name(font_name, font_size).unwrap_or_else(|_| {
        ct_font::new_from_name("Menlo", font_size).expect("Failed to create fallback font")
    });

    // 创建 attributed string
    let cf_content = core_foundation::string::CFString::new(&item.content);
    let mut attr_str = CFMutableAttributedString::new();

    let zero_range = core_foundation::base::CFRange::init(0, 0);
    attr_str.replace_str(&cf_content, zero_range);

    let full_range = core_foundation::base::CFRange::init(0, cf_content.char_len());

    // 设置字体属性
    unsafe {
        let key = core_text::string_attributes::kCTFontAttributeName;
        attr_str.set_attribute(full_range, key, &ct_font);
    }

    // 设置颜色
    let r = item.color[0] as f64;
    let g = item.color[1] as f64;
    let b = item.color[2] as f64;
    let a = item.color[3] as f64;
    context.set_rgb_fill_color(r, g, b, a);

    unsafe {
        let key = core_text::string_attributes::kCTForegroundColorFromContextAttributeName;
        let yes = CFNumber::from(1i32);
        attr_str.set_attribute(full_range, key, &yes);
    }

    // 创建 CTLine
    let attr_ref = attr_str.as_concrete_TypeRef();
    let line = CTLine::new_with_attributed_string(attr_ref as *const _);

    // 测量文字用于垂直居中
    let bounds = line.get_typographic_bounds();
    let text_height = bounds.ascent + bounds.descent;

    // UI 坐标 -> CG 坐标 (翻转 Y)
    // UI: y_ui 是从顶部到文字区域顶部的距离
    // CG: y_cg 是从底部到 baseline 的距离
    let vertical_center_offset = (line_height - text_height) / 2.0;
    let baseline_y_cg = viewport_height - y_ui - line_height + vertical_center_offset + bounds.ascent;

    // 裁剪到最大宽度
    // CG 裁剪区域也需要翻转 Y
    let clip_y_cg = viewport_height - y_ui - line_height;
    context.save();
    context.clip_to_rect(core_graphics::geometry::CGRect::new(
        &core_graphics::geometry::CGPoint::new(x, clip_y_cg),
        &core_graphics::geometry::CGSize::new(max_width, line_height),
    ));

    // 绘制文字
    context.set_text_position(x, baseline_y_cg);
    line.draw(context);

    context.restore();
}
