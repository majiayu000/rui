use crate::ImageFit;
use crate::core::geometry::{Bounds, Size};
use crate::core::style::Corners;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct GpuImage {
    pub bounds: [f32; 4],
    pub corner_radii: [f32; 4],
    pub color: [f32; 4],
    pub opacity: f32,
    pub _padding: [f32; 3],
}

impl GpuImage {
    pub(super) fn new(
        bounds: Bounds,
        corner_radii: Corners,
        color: [f32; 4],
        opacity: f32,
    ) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            color,
            opacity,
            _padding: [0.0; 3],
        }
    }
}

pub(super) fn calculate_fit_bounds(container: Bounds, image_size: Size, fit: ImageFit) -> Bounds {
    if image_size.width <= 0.0 || image_size.height <= 0.0 {
        return container;
    }
    match fit {
        ImageFit::Fill => container,
        ImageFit::Contain => {
            let scale_x = container.width() / image_size.width;
            let scale_y = container.height() / image_size.height;
            let scale = scale_x.min(scale_y);
            let width = image_size.width * scale;
            let height = image_size.height * scale;
            let x = container.x() + (container.width() - width) / 2.0;
            let y = container.y() + (container.height() - height) / 2.0;
            Bounds::from_xywh(x, y, width, height)
        }
        ImageFit::Cover => {
            let scale_x = container.width() / image_size.width;
            let scale_y = container.height() / image_size.height;
            let scale = scale_x.max(scale_y);
            let width = image_size.width * scale;
            let height = image_size.height * scale;
            let x = container.x() + (container.width() - width) / 2.0;
            let y = container.y() + (container.height() - height) / 2.0;
            Bounds::from_xywh(x, y, width, height)
        }
        ImageFit::None => {
            let x = container.x() + (container.width() - image_size.width) / 2.0;
            let y = container.y() + (container.height() - image_size.height) / 2.0;
            Bounds::from_xywh(x, y, image_size.width, image_size.height)
        }
        ImageFit::ScaleDown => {
            if image_size.width <= container.width() && image_size.height <= container.height() {
                let x = container.x() + (container.width() - image_size.width) / 2.0;
                let y = container.y() + (container.height() - image_size.height) / 2.0;
                Bounds::from_xywh(x, y, image_size.width, image_size.height)
            } else {
                let scale_x = container.width() / image_size.width;
                let scale_y = container.height() / image_size.height;
                let scale = scale_x.min(scale_y);
                let width = image_size.width * scale;
                let height = image_size.height * scale;
                let x = container.x() + (container.width() - width) / 2.0;
                let y = container.y() + (container.height() - height) / 2.0;
                Bounds::from_xywh(x, y, width, height)
            }
        }
    }
}
