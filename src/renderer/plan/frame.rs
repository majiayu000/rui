//! A frame's draws, planned before any is encoded.
//!
//! Primitives become items: a run of sprite instances (quads, shadows, glyphs,
//! paths) or an image, each with the area it can paint. Sprites of every kind
//! share one pipeline, so a run of them is one draw unless glyphs on another
//! atlas page or an image comes between. When one does, items are drawn
//! by overlap order instead of scene order: everything that overlaps nothing
//! earlier goes first, then what overlaps only those, and so on, and sprites of a
//! level join the open sprite draw. Items of one level never overlap, so any
//! sequence among them paints the same pixels.
//!
//! Simple sprites, glyphs and pixel-aligned plain quads that lie inside their
//! clip, have a smaller fragment function of their own. A draw of them ends
//! where another sprite comes; an open draw of other sprites takes simple ones
//! too, as its fragment function draws every kind, so draws never split more
//! often than they would without it.

use super::order::{OverlapOrder, Rect};

pub(crate) const KIND_QUAD: u32 = 0;
pub(crate) const KIND_SHADOW: u32 = 1;
pub(crate) const KIND_GLYPH: u32 = 2;
pub(crate) const KIND_PATH: u32 = 3;
pub(crate) const KIND_PATH_SPRITE: u32 = 4;
pub(crate) const KIND_COLOR_GLYPH: u32 = 5;

/// The atlas pages a run of sprites samples: a coverage page and a color page,
/// each bound to its own texture slot, so sprites on one page of each share a
/// draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pages {
    pub coverage: Option<u32>,
    pub color: Option<u32>,
}

impl Pages {
    pub(crate) fn coverage(page: u32) -> Self {
        Self {
            coverage: Some(page),
            color: None,
        }
    }

    pub(crate) fn color(page: u32) -> Self {
        Self {
            coverage: None,
            color: Some(page),
        }
    }

    /// Whether one draw can sample both: no slot needs two pages.
    fn fits(self, other: Self) -> bool {
        let fits = |a: Option<u32>, b: Option<u32>| a.is_none() || b.is_none() || a == b;
        fits(self.coverage, other.coverage) && fits(self.color, other.color)
    }

    fn with(self, other: Self) -> Self {
        Self {
            coverage: self.coverage.or(other.coverage),
            color: self.color.or(other.color),
        }
    }
}

/// One sprite instance as the shader reads it: `kind << 29 | index` into that
/// kind's buffer, and the clip rectangle to apply.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SpriteRef {
    pub index: u32,
    pub clip: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    /// `count` instances of `kind` from `start`, all sampling atlas `pages`;
    /// `simple` when all of them can draw with the simple pipeline.
    Sprites {
        kind: u32,
        start: u32,
        count: u32,
        pages: Pages,
        simple: bool,
    },
    /// The renderer's image draw at this index.
    Image(u32),
}

#[derive(Clone, Copy, Debug)]
struct Item {
    kind: ItemKind,
    clip: u32,
    rect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Draw {
    /// Sprite references `start..start + count`, sampling atlas `pages`, with
    /// the simple pipeline when `simple`.
    Sprites {
        pages: Pages,
        start: u32,
        count: u32,
        simple: bool,
    },
    Image(u32),
}

#[derive(Default)]
pub(crate) struct FramePlan {
    items: Vec<Item>,
    /// Whether scene order needs more than one draw.
    mixed: bool,
    /// Draw in scene order even when mixed: the plain path the tests compare with.
    #[cfg(test)]
    pub scene_order: bool,
    pages: Pages,
    order: OverlapOrder,
    orders: Vec<u32>,
    by_order: Vec<u32>,
    pending: Vec<u32>,
    pub refs: Vec<SpriteRef>,
    pub draws: Vec<Draw>,
}

impl FramePlan {
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.mixed = false;
        self.pages = Pages::default();
        self.refs.clear();
        self.draws.clear();
    }

    /// Adds an item that paints only within `rect`, logical pixels clipped to
    /// what can be seen. Without one, `plan` asks for it if it needs it.
    pub(crate) fn push(&mut self, kind: ItemKind, clip: u32, rect: Option<Rect>) {
        match kind {
            ItemKind::Sprites { pages, .. } if self.pages.fits(pages) => {
                self.pages = self.pages.with(pages);
            }
            ItemKind::Sprites { .. } | ItemKind::Image(_) => self.mixed = true,
        }
        self.items.push(Item { kind, clip, rect });
    }

    /// Fills `refs` and `draws`. `width` and `height` bound every item's rect;
    /// `rect_of` gives those pushed without one, `None` when nothing is visible.
    pub(crate) fn plan(
        &mut self,
        width: f32,
        height: f32,
        mut rect_of: impl FnMut(ItemKind, u32) -> Option<Rect>,
    ) {
        #[cfg(test)]
        let mixed = self.mixed && !self.scene_order;
        #[cfg(not(test))]
        let mixed = self.mixed;
        if !mixed {
            for index in 0..self.items.len() {
                self.append(index);
            }
            return;
        }

        self.order.reset(width, height);
        self.orders.clear();
        let mut levels = 0;
        for item in &self.items {
            let rect = item
                .rect
                .or_else(|| rect_of(item.kind, item.clip))
                .unwrap_or_default();
            let order = self.order.insert(rect);
            levels = levels.max(order);
            self.orders.push(order);
        }
        // Counting sort by order, keeping scene order within a level.
        let mut starts = vec![0u32; levels as usize + 2];
        for &order in &self.orders {
            starts[order as usize + 1] += 1;
        }
        for level in 1..starts.len() {
            starts[level] += starts[level - 1];
        }
        self.by_order.clear();
        self.by_order.resize(self.items.len(), 0);
        let mut next = starts.clone();
        for (index, &order) in self.orders.iter().enumerate() {
            self.by_order[next[order as usize] as usize] = index as u32;
            next[order as usize] += 1;
        }

        let mut pending = std::mem::take(&mut self.pending);
        for level in 1..=levels as usize {
            pending.clear();
            pending.extend_from_slice(
                &self.by_order[starts[level] as usize..starts[level + 1] as usize],
            );
            loop {
                pending.retain(|&index| !self.join(index as usize));
                let Some(&first) = pending.first() else {
                    break;
                };
                pending.remove(0);
                self.append(first as usize);
            }
        }
        self.pending = pending;
    }

    /// Adds the item to the open draw if it can share it.
    fn join(&mut self, index: usize) -> bool {
        let item = self.items[index];
        let ItemKind::Sprites { pages, simple, .. } = item.kind else {
            return false;
        };
        match self.draws.last() {
            Some(&Draw::Sprites {
                pages: open,
                simple: open_simple,
                ..
            }) if open.fits(pages) && takes(open_simple, simple) => {
                self.append(index);
                true
            }
            _ => false,
        }
    }

    /// Adds the item to the open draw when it fits, else opens a draw for it.
    fn append(&mut self, index: usize) {
        let item = self.items[index];
        match item.kind {
            ItemKind::Sprites {
                kind,
                start,
                count,
                pages,
                simple,
            } => {
                let refs = self.refs.len() as u32;
                self.refs
                    .extend((start..start + count).map(|index| SpriteRef {
                        index: kind << 29 | index,
                        clip: item.clip,
                    }));
                match self.draws.last_mut() {
                    Some(Draw::Sprites {
                        pages: open,
                        count: open_count,
                        simple: open_simple,
                        ..
                    }) if open.fits(pages) && takes(*open_simple, simple) => {
                        *open = open.with(pages);
                        *open_count += count;
                    }
                    _ => self.draws.push(Draw::Sprites {
                        pages,
                        start: refs,
                        count,
                        simple,
                    }),
                }
            }
            ItemKind::Image(image) => self.draws.push(Draw::Image(image)),
        }
    }
}

/// Whether an open draw, simple or not, can take a sprite that is: one of
/// the simple pipeline takes only simple sprites.
fn takes(open_simple: bool, simple: bool) -> bool {
    simple || !open_simple
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        [x, y, x + width, y + height]
    }

    fn quad(plan: &mut FramePlan, index: u32, rect: Rect) {
        plan.push(
            ItemKind::Sprites {
                kind: KIND_QUAD,
                start: index,
                count: 1,
                pages: Pages::default(),
                simple: false,
            },
            0,
            Some(rect),
        );
    }

    fn glyphs(plan: &mut FramePlan, start: u32, count: u32, page: u32, rect: Rect) {
        plan.push(
            ItemKind::Sprites {
                kind: KIND_GLYPH,
                start,
                count,
                pages: Pages::coverage(page),
                simple: false,
            },
            0,
            Some(rect),
        );
    }

    #[test]
    fn sprites_on_one_page_draw_once_in_scene_order() {
        let mut plan = FramePlan::default();
        for row in 0..40 {
            let y = row as f32 * 28.0;
            quad(&mut plan, row, rect(0.0, y, 1440.0, 28.0));
            glyphs(&mut plan, row * 10, 10, 0, rect(8.0, y + 6.0, 200.0, 16.0));
        }
        plan.plan(1440.0, 900.0, |_, _| None);
        assert_eq!(
            plan.draws,
            [Draw::Sprites {
                pages: Pages::coverage(0),
                start: 0,
                count: 440,
                simple: false
            }]
        );
        assert_eq!(plan.refs[0].index, KIND_QUAD << 29);
        assert_eq!(plan.refs[1].index, KIND_GLYPH << 29);
        assert_eq!(plan.refs[11].index, KIND_QUAD << 29 | 1);
    }

    #[test]
    fn an_image_splits_only_what_overlaps_it() {
        let mut plan = FramePlan::default();
        // A background, then rows of quads and text, an image over the first row.
        quad(&mut plan, 0, rect(0.0, 0.0, 400.0, 400.0));
        for row in 0..10u32 {
            let y = row as f32 * 40.0;
            quad(&mut plan, 1 + row, rect(10.0, y + 5.0, 300.0, 30.0));
            glyphs(&mut plan, row * 4, 4, 0, rect(20.0, y + 10.0, 100.0, 20.0));
            if row == 0 {
                plan.push(ItemKind::Image(0), 0, Some(rect(320.0, 5.0, 30.0, 30.0)));
            }
        }
        plan.plan(400.0, 400.0, |_, _| None);
        // The background, all row quads, the image, then all text.
        assert_eq!(
            plan.draws,
            [
                Draw::Sprites {
                    pages: Pages::default(),
                    start: 0,
                    count: 11,
                    simple: false
                },
                Draw::Image(0),
                Draw::Sprites {
                    pages: Pages::coverage(0),
                    start: 11,
                    count: 40,
                    simple: false
                },
            ]
        );
    }

    #[test]
    fn overlapping_items_keep_scene_order_across_pages() {
        let mut plan = FramePlan::default();
        glyphs(&mut plan, 0, 1, 0, rect(0.0, 0.0, 10.0, 10.0));
        glyphs(&mut plan, 1, 1, 1, rect(5.0, 0.0, 10.0, 10.0));
        glyphs(&mut plan, 2, 1, 0, rect(10.0, 0.0, 10.0, 10.0));
        // Apart from all of them, on page 1: joins the page 1 draw.
        glyphs(&mut plan, 3, 1, 1, rect(100.0, 0.0, 10.0, 10.0));
        plan.plan(200.0, 200.0, |_, _| None);
        assert_eq!(
            plan.draws,
            [
                Draw::Sprites {
                    pages: Pages::coverage(0),
                    start: 0,
                    count: 1,
                    simple: false
                },
                Draw::Sprites {
                    pages: Pages::coverage(1),
                    start: 1,
                    count: 2,
                    simple: false
                },
                Draw::Sprites {
                    pages: Pages::coverage(0),
                    start: 3,
                    count: 1,
                    simple: false
                },
            ]
        );
        let order: Vec<u32> = plan.refs.iter().map(|r| r.index & 0x1fff_ffff).collect();
        assert_eq!(order, [0, 3, 1, 2]);
    }

    #[test]
    fn simple_sprites_join_an_open_draw_of_others() {
        let mut plan = FramePlan::default();
        for (start, simple) in [(0, true), (1, true), (2, false), (3, true)] {
            plan.push(
                ItemKind::Sprites {
                    kind: KIND_QUAD,
                    start,
                    count: 1,
                    pages: Pages::default(),
                    simple,
                },
                0,
                Some(rect(0.0, 0.0, 10.0, 10.0)),
            );
        }
        plan.plan(100.0, 100.0, |_, _| None);
        assert_eq!(
            plan.draws,
            [
                Draw::Sprites {
                    pages: Pages::default(),
                    start: 0,
                    count: 2,
                    simple: true
                },
                Draw::Sprites {
                    pages: Pages::default(),
                    start: 2,
                    count: 2,
                    simple: false
                },
            ]
        );
    }

    #[test]
    fn clear_starts_a_new_frame() {
        let mut plan = FramePlan::default();
        plan.push(ItemKind::Image(0), 0, Some(rect(0.0, 0.0, 10.0, 10.0)));
        plan.plan(100.0, 100.0, |_, _| None);
        plan.clear();
        assert!(plan.draws.is_empty() && plan.refs.is_empty());
        quad(&mut plan, 0, rect(0.0, 0.0, 10.0, 10.0));
        plan.plan(100.0, 100.0, |_, _| None);
        assert_eq!(plan.draws.len(), 1);
    }
}
