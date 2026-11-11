//! Text that elements hand to the scene without copying it.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, Range};
use std::sync::{Arc, LazyLock};

/// A range of a reference-counted string.
///
/// Cloning or slicing one never copies the text, so an element can paint the
/// same text, or pieces of it, every frame without allocating.
#[derive(Clone)]
pub struct SharedStr {
    text: Arc<str>,
    range: Range<usize>,
}

static EMPTY: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(""));

impl SharedStr {
    pub fn new(text: Arc<str>) -> Self {
        let range = 0..text.len();
        Self { text, range }
    }

    /// `range` of `text`. Panics when it is out of bounds or not on a char
    /// boundary, as indexing a `str` does.
    pub fn slice(text: &Arc<str>, range: Range<usize>) -> Self {
        let _ = &text[range.clone()];
        Self {
            text: Arc::clone(text),
            range,
        }
    }

    /// `range` of this text, relative to its start.
    pub fn sub(&self, range: Range<usize>) -> Self {
        let _ = &self.as_str()[range.clone()];
        Self {
            text: Arc::clone(&self.text),
            range: self.range.start + range.start..self.range.start + range.end,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text[self.range.clone()]
    }
}

impl Default for SharedStr {
    fn default() -> Self {
        Self::new(Arc::clone(&EMPTY))
    }
}

impl Deref for SharedStr {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for SharedStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for SharedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for SharedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl PartialEq for SharedStr {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SharedStr {}

impl PartialEq<str> for SharedStr {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SharedStr {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for SharedStr {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl Hash for SharedStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl From<Arc<str>> for SharedStr {
    fn from(text: Arc<str>) -> Self {
        Self::new(text)
    }
}

impl From<&Arc<str>> for SharedStr {
    fn from(text: &Arc<str>) -> Self {
        Self::new(Arc::clone(text))
    }
}

impl From<String> for SharedStr {
    fn from(text: String) -> Self {
        Self::new(Arc::from(text))
    }
}

impl From<&String> for SharedStr {
    fn from(text: &String) -> Self {
        Self::new(Arc::from(text.as_str()))
    }
}

impl From<&str> for SharedStr {
    fn from(text: &str) -> Self {
        Self::new(Arc::from(text))
    }
}

impl From<char> for SharedStr {
    fn from(c: char) -> Self {
        Self::new(Arc::from(c.to_string()))
    }
}

impl From<SharedStr> for String {
    fn from(text: SharedStr) -> Self {
        text.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_share_the_text() {
        let text: Arc<str> = Arc::from("hello world");
        let word = SharedStr::slice(&text, 6..11);
        assert_eq!(word, "world");
        assert_eq!(word.sub(1..3), "or");
        assert_eq!(Arc::strong_count(&text), 2);
    }

    #[test]
    #[should_panic]
    fn slice_off_a_char_boundary_panics() {
        let text: Arc<str> = Arc::from("é");
        let _ = SharedStr::slice(&text, 0..1);
    }
}
