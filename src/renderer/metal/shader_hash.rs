//! The hash naming compiled shaders. `build.rs` includes this file too, so a
//! library it embeds carries the same hash of its source as the runtime computes.

/// FNV-1a over `parts`, each followed by a zero byte so their boundaries count.
pub(crate) fn shader_hash(parts: &[&[u8]]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for part in parts {
        for &byte in part.iter().chain(&[0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}
