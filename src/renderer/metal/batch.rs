//! Binding per-draw data without a buffer for each draw.

use metal::{DeviceRef, MTLResourceOptions, RenderCommandEncoderRef};
use std::mem;

/// `setVertexBytes` takes at most 4 KB.
const MAX_INLINE_VERTEX_BYTES: u64 = 4096;

pub(super) fn bind_vertex_value<T: Copy>(encoder: &RenderCommandEncoderRef, index: u64, value: &T) {
    encoder.set_vertex_bytes(
        index,
        mem::size_of::<T>() as u64,
        value as *const T as *const _,
    );
}

/// Small data goes inline with the encoder; larger data gets its own buffer.
pub(super) fn bind_vertex_slice<T: Copy>(
    device: &DeviceRef,
    encoder: &RenderCommandEncoderRef,
    index: u64,
    values: &[T],
) {
    let bytes = mem::size_of_val(values) as u64;
    if bytes <= MAX_INLINE_VERTEX_BYTES {
        encoder.set_vertex_bytes(index, bytes, values.as_ptr() as *const _);
    } else {
        let buffer = device.new_buffer_with_data(
            values.as_ptr() as *const _,
            bytes,
            MTLResourceOptions::CPUCacheModeDefaultCache,
        );
        encoder.set_vertex_buffer(index, Some(&buffer), 0);
    }
}
