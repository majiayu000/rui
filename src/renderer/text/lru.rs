//! A bounded map that evicts what recent frames used last.
//!
//! Entries are found by a hash of a borrowed request and confirmed by comparing
//! the stored key, so a hit allocates nothing. Every access stamps the entry with
//! a tick. [`FrameLru::begin_frame`] trims the map back to its target size, but
//! only from entries older than the previous frame, so text that is still on
//! screen is never evicted for being numerous. A trim goes below the target by
//! an eighth, so text that keeps arriving, as in a scrolling view, is trimmed
//! every few hundred entries rather than every frame. The hard limits hold at
//! all times: past them the least recently used entries go, whatever frame used
//! them.

use super::hash::FastMap;

struct Slot<K, V> {
    key: K,
    value: V,
    bytes: usize,
    last_used: u64,
}

pub(super) struct FrameLru<K, V> {
    /// By the key's hash, which the map uses as it is.
    entries: FastMap<u64, Slot<K, V>>,
    bytes: usize,
    tick: u64,
    /// First tick of the frame in progress.
    frame_start: u64,
    target_entries: usize,
    target_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
}

impl<K, V> FrameLru<K, V> {
    pub(super) fn new(
        target_entries: usize,
        target_bytes: usize,
        max_entries: usize,
        max_bytes: usize,
    ) -> Self {
        Self {
            entries: FastMap::default(),
            bytes: 0,
            tick: 0,
            frame_start: 0,
            target_entries,
            target_bytes,
            max_entries,
            max_bytes,
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub(super) fn for_each_value(&mut self, mut f: impl FnMut(&mut V)) {
        self.entries
            .values_mut()
            .for_each(|slot| f(&mut slot.value));
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    /// Past the target size, trim to an eighth below it, from entries no frame
    /// since the previous one used.
    pub(super) fn begin_frame(&mut self) {
        let previous_frame = self.frame_start;
        if self.entries.len() > self.target_entries || self.bytes > self.target_bytes {
            self.evict(
                self.target_entries - self.target_entries / 8,
                self.target_bytes - self.target_bytes / 8,
                previous_frame,
            );
        }
        self.frame_start = self.tick;
    }

    pub(super) fn get_mut(&mut self, hash: u64, matches: impl Fn(&K) -> bool) -> Option<&mut V> {
        let slot = self.entries.get_mut(&hash)?;
        if !matches(&slot.key) {
            return None;
        }
        self.tick += 1;
        slot.last_used = self.tick;
        Some(&mut slot.value)
    }

    pub(super) fn remove(&mut self, hash: u64, matches: impl Fn(&K) -> bool) -> Option<V> {
        if !self
            .entries
            .get(&hash)
            .is_some_and(|slot| matches(&slot.key))
        {
            return None;
        }
        let slot = self.entries.remove(&hash)?;
        self.bytes -= slot.bytes;
        Some(slot.value)
    }

    /// Keep `value`, replacing whatever had the same hash. A value bigger than
    /// the whole byte limit is not kept.
    pub(super) fn insert(&mut self, hash: u64, key: K, value: V, bytes: usize) {
        if bytes > self.max_bytes {
            return;
        }
        if let Some(old) = self.entries.remove(&hash) {
            self.bytes -= old.bytes;
        }
        if self.entries.len() >= self.max_entries
            || self.bytes.saturating_add(bytes) > self.max_bytes
        {
            let entries = self.max_entries - self.max_entries / 4;
            let bytes_room = (self.max_bytes - self.max_bytes / 4).min(self.max_bytes - bytes);
            self.evict(entries, bytes_room, u64::MAX);
        }
        self.tick += 1;
        self.bytes += bytes;
        self.entries.insert(
            hash,
            Slot {
                key,
                value,
                bytes,
                last_used: self.tick,
            },
        );
    }

    /// Drop the least recently used entries until at most `entries` and `bytes`
    /// remain, never one used at or after tick `keep_from`.
    fn evict(&mut self, entries: usize, bytes: usize, keep_from: u64) {
        let mut candidates: Vec<(u64, usize)> = self
            .entries
            .values()
            .filter(|slot| slot.last_used < keep_from)
            .map(|slot| (slot.last_used, slot.bytes))
            .collect();
        candidates.sort_unstable_by_key(|&(last_used, _)| last_used);

        let mut remaining = self.entries.len();
        let mut remaining_bytes = self.bytes;
        let mut threshold = None;
        for (last_used, slot_bytes) in candidates {
            if remaining <= entries && remaining_bytes <= bytes {
                break;
            }
            remaining -= 1;
            remaining_bytes -= slot_bytes;
            threshold = Some(last_used);
        }
        let Some(threshold) = threshold else {
            return;
        };
        self.entries.retain(|_, slot| slot.last_used > threshold);
        self.bytes = remaining_bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::FrameLru;

    fn lru() -> FrameLru<u64, ()> {
        FrameLru::new(4, usize::MAX, 8, usize::MAX)
    }

    fn touch(cache: &mut FrameLru<u64, ()>, key: u64) -> bool {
        if cache.get_mut(key, |stored| *stored == key).is_some() {
            return true;
        }
        cache.insert(key, key, (), 1);
        false
    }

    #[test]
    fn frames_trim_only_entries_older_than_the_previous_frame() {
        let mut cache = lru();
        cache.begin_frame();
        for key in 0..6 {
            touch(&mut cache, key);
        }
        cache.begin_frame();
        assert_eq!(cache.len(), 6, "the frame that just drew keeps its text");
        for key in 4..6 {
            assert!(touch(&mut cache, key));
        }
        cache.begin_frame();
        assert_eq!(cache.len(), 4);
        for key in 2..6 {
            assert!(touch(&mut cache, key), "{key} was used most recently");
        }
    }

    #[test]
    fn text_that_keeps_arriving_is_trimmed_every_few_frames() {
        let mut cache: FrameLru<u64, ()> = FrameLru::new(16, usize::MAX, 64, usize::MAX);
        for key in 0..20 {
            touch(&mut cache, key);
        }
        cache.begin_frame();
        cache.begin_frame();
        assert_eq!(cache.len(), 14, "an eighth below the target");
        // One new entry per frame, as a view scrolling a line at a time.
        for key in 20..22 {
            touch(&mut cache, key);
            cache.begin_frame();
            assert_eq!(cache.len(), key as usize - 5, "under the target: no trim");
        }
        touch(&mut cache, 22);
        cache.begin_frame();
        assert_eq!(cache.len(), 14, "past the target: trimmed again");
    }

    #[test]
    fn the_hard_limit_evicts_least_recently_used_first() {
        let mut cache = lru();
        for key in 0..8 {
            touch(&mut cache, key);
        }
        assert!(touch(&mut cache, 0));
        touch(&mut cache, 8);
        assert!(cache.len() <= 8);
        assert!(touch(&mut cache, 0), "used recently, kept");
        assert!(!touch(&mut cache, 1), "least recently used, evicted");
    }

    #[test]
    fn a_hash_collision_is_a_miss_not_a_wrong_hit() {
        let mut cache: FrameLru<&str, u32> = FrameLru::new(4, 64, 8, 64);
        cache.insert(7, "a", 1, 1);
        assert!(cache.get_mut(7, |key| *key == "b").is_none());
        assert!(cache.remove(7, |key| *key == "b").is_none());
        assert_eq!(cache.get_mut(7, |key| *key == "a").copied(), Some(1));
    }

    #[test]
    fn bytes_stay_within_the_limit() {
        let mut cache: FrameLru<u64, ()> = FrameLru::new(4, 10, 100, 20);
        for key in 0..50 {
            cache.insert(key, key, (), 3);
            assert!(cache.bytes() <= 20);
        }
        cache.insert(99, 99, (), 21);
        assert!(cache.get_mut(99, |key| *key == 99).is_none());
    }
}
