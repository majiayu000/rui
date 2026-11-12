//! [`ElementBox`]: the box an [`AnyElement`] keeps its element in, whose
//! memory is reused for the next element of the same layout.
//!
//! A tree rebuilt every frame drops as many elements as it builds, mostly of a
//! few types. The allocator's malloc and free cost several hundred instructions
//! a pair; handing the memory of a dropped element to the next element of that
//! size costs a few dozen.

use super::Element;
use std::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// Most bytes of dropped elements one thread keeps for reuse; past it, they
/// are freed. Enough for a frame of a few thousand rows.
const MAX_POOLED_BYTES: usize = 8 << 20;

/// Freed element memory by layout. Few element types are common, so a list
/// searched in order finds them faster than a map.
struct Pool {
    classes: Vec<(Layout, Vec<NonNull<u8>>)>,
    bytes: usize,
}

impl Pool {
    fn take(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let (_, blocks) = self.classes.iter_mut().find(|(at, _)| *at == layout)?;
        let block = blocks.pop()?;
        self.bytes -= layout.size();
        Some(block)
    }

    /// Keeps `block`, or hands it back when the pool is full.
    fn keep(&mut self, layout: Layout, block: NonNull<u8>) -> Result<(), NonNull<u8>> {
        if self.bytes + layout.size() > MAX_POOLED_BYTES {
            return Err(block);
        }
        let index = match self.classes.iter().position(|(at, _)| *at == layout) {
            Some(index) => index,
            None => {
                self.classes.push((layout, Vec::new()));
                self.classes.len() - 1
            }
        };
        self.classes[index].1.push(block);
        self.bytes += layout.size();
        Ok(())
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        for (layout, blocks) in &self.classes {
            for block in blocks {
                // SAFETY: every block was allocated with `layout`; see `ElementBox::new`.
                unsafe { dealloc(block.as_ptr(), *layout) };
            }
        }
    }
}

thread_local! {
    static POOL: RefCell<Pool> = const {
        RefCell::new(Pool {
            classes: Vec::new(),
            bytes: 0,
        })
    };
}

/// An owned `dyn Element` on the heap, as a `Box` would hold it.
pub(super) struct ElementBox(NonNull<dyn Element>);

impl ElementBox {
    pub(super) fn new<E: Element>(element: E) -> Self {
        let layout = Layout::new::<E>();
        if layout.size() == 0 {
            return Self(NonNull::from(Box::leak(
                Box::new(element) as Box<dyn Element>
            )));
        }
        let block = POOL
            .try_with(|pool| pool.borrow_mut().take(layout))
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                // SAFETY: `layout` has a non-zero size.
                NonNull::new(unsafe { alloc(layout) }).unwrap_or_else(|| handle_alloc_error(layout))
            });
        let element_ptr = block.cast::<E>();
        // SAFETY: the block is fresh or freed memory of `E`'s layout.
        unsafe { element_ptr.as_ptr().write(element) };
        Self(element_ptr as NonNull<dyn Element>)
    }
}

impl Deref for ElementBox {
    type Target = dyn Element;

    fn deref(&self) -> &Self::Target {
        // SAFETY: the pointer owns a live element until `drop`.
        unsafe { self.0.as_ref() }
    }
}

impl DerefMut for ElementBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: as in `deref`, and `&mut self` makes the access unique.
        unsafe { self.0.as_mut() }
    }
}

impl Drop for ElementBox {
    fn drop(&mut self) {
        let layout = Layout::for_value::<dyn Element>(&**self);
        // SAFETY: the element is live and dropped once; its memory stays
        // allocated with `layout` until freed below or reused.
        unsafe { self.0.as_ptr().drop_in_place() };
        if layout.size() == 0 {
            return;
        }
        let block = self.0.cast::<u8>();
        // Dropping the element above may have used the pool; it is free again.
        let kept = POOL.try_with(|pool| pool.borrow_mut().keep(layout, block));
        if !matches!(kept, Ok(Ok(()))) {
            // SAFETY: allocated with `layout` and no longer used.
            unsafe { dealloc(block.as_ptr(), layout) };
        }
    }
}
