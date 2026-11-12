//! Binding per-draw data without a buffer for each draw.

use metal::{
    Buffer, BufferRef, CommandBuffer, CommandBufferRef, DeviceRef, MTLCommandBufferStatus,
    MTLResourceOptions, RenderCommandEncoderRef,
};
use std::mem;

pub(super) fn bind_vertex_value<T: Copy>(encoder: &RenderCommandEncoderRef, index: u64, value: &T) {
    encoder.set_vertex_bytes(
        index,
        mem::size_of::<T>() as u64,
        value as *const T as *const _,
    );
}

/// Offsets within a buffer are kept to this, which every Metal GPU accepts for
/// constant address space buffers.
const BUFFER_OFFSET_ALIGNMENT: usize = 256;

/// Smallest instance buffer allocated, so small frames reuse one buffer.
const MIN_INSTANCE_BUFFER_BYTES: usize = 64 * 1024;

/// Instance buffers reused across frames, as GPUI pools them. Each frame copies
/// its instance data into one buffer, which goes back to the pool once the GPU
/// has finished the command buffer that read it. The pool holds as many buffers
/// as frames were ever in flight at once.
#[derive(Default)]
pub(super) struct InstancePool {
    free: Vec<Buffer>,
    in_flight: Vec<(CommandBuffer, Buffer)>,
}

impl InstancePool {
    /// A buffer of at least `bytes` that no unfinished command buffer reads.
    pub(super) fn acquire(&mut self, device: &DeviceRef, bytes: usize) -> Buffer {
        let mut index = 0;
        while index < self.in_flight.len() {
            if matches!(
                self.in_flight[index].0.status(),
                MTLCommandBufferStatus::Completed | MTLCommandBufferStatus::Error
            ) {
                let (_, buffer) = self.in_flight.swap_remove(index);
                self.free.push(buffer);
            } else {
                index += 1;
            }
        }
        let fit = self
            .free
            .iter()
            .enumerate()
            .filter(|(_, buffer)| buffer.length() as usize >= bytes)
            .min_by_key(|(_, buffer)| buffer.length())
            .map(|(index, _)| index);
        match fit {
            Some(index) => self.free.swap_remove(index),
            None => {
                // Every free buffer is too small for this frame and later ones like it.
                self.free.clear();
                device.new_buffer(
                    bytes.max(MIN_INSTANCE_BUFFER_BYTES).next_power_of_two() as u64,
                    MTLResourceOptions::CPUCacheModeWriteCombined
                        | MTLResourceOptions::StorageModeShared,
                )
            }
        }
    }

    /// Returns `buffer` to the pool once `command_buffer` has completed.
    pub(super) fn retire(&mut self, command_buffer: &CommandBufferRef, buffer: Buffer) {
        self.in_flight.push((command_buffer.to_owned(), buffer));
    }
}

/// Lays slices out one after another in one buffer.
#[derive(Default)]
pub(super) struct BufferLayout {
    bytes: usize,
}

impl BufferLayout {
    /// Reserves room for `values` and returns its offset.
    pub(super) fn reserve<T>(&mut self, values: &[T]) -> usize {
        let offset = self.bytes.next_multiple_of(BUFFER_OFFSET_ALIGNMENT);
        self.bytes = offset + mem::size_of_val(values).max(1);
        offset
    }

    /// Reserves room for `count` values of `T` and returns its offset.
    pub(super) fn reserve_count<T>(&mut self, count: usize) -> usize {
        let offset = self.bytes.next_multiple_of(BUFFER_OFFSET_ALIGNMENT);
        self.bytes = offset + (count * mem::size_of::<T>()).max(1);
        offset
    }

    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Copies `values` into `buffer` at `offset`, which `BufferLayout` reserved.
pub(super) fn write_slice<T: Copy>(buffer: &BufferRef, offset: usize, values: &[T]) {
    let bytes = mem::size_of_val(values);
    assert!(
        offset + bytes <= buffer.length() as usize,
        "instance data overruns its buffer"
    );
    // SAFETY: the range is within the buffer, checked above, and the source is a
    // slice of plain `Copy` data that cannot overlap the buffer's memory.
    unsafe {
        std::ptr::copy_nonoverlapping(
            values.as_ptr() as *const u8,
            (buffer.contents() as *mut u8).add(offset),
            bytes,
        );
    }
}
