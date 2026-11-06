//! The hasher of the text caches' keys.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A folded-multiply hash, as foldhash and wyhash mix: far cheaper than the
/// default SipHash for the keys the text caches look up for every glyph drawn
/// and every text measured. Folding the 128-bit product's high half into the
/// low half keeps strings that differ in two words apart; rustc's FxHash step
/// (rotate, xor, multiply) let `task 1492: …` and `task 1419: …` collide, and a
/// cache that keeps one entry per hash turns each collision into a miss. Not
/// resistant to chosen collisions, which only cost time here: entries are
/// compared in full.
#[derive(Default, Clone, Copy)]
pub(super) struct FastHasher(u64);

impl FastHasher {
    fn add(&mut self, word: u64) {
        let product = u128::from(self.0 ^ word) * u128::from(0x51_7c_c1_b7_27_22_0a_95_u64);
        self.0 = product as u64 ^ (product >> 64) as u64;
    }
}

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.add(u64::from_le_bytes(*word));
        }
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

pub(super) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;

#[cfg(test)]
mod tests {
    use super::FastHasher;
    use std::collections::HashSet;
    use std::hash::{Hash, Hasher};

    fn hash(text: &str) -> u64 {
        let mut hasher = FastHasher::default();
        text.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn strings_that_differ_in_two_words_hash_apart() {
        assert_ne!(
            hash("task 1492: fix the failing check"),
            hash("task 1419: fix the failing check")
        );
        let hashes: HashSet<u64> = (0..100_000)
            .map(|index| hash(&format!("task {index}: fix the failing check")))
            .collect();
        assert_eq!(hashes.len(), 100_000);
    }
}
