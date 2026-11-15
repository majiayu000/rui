use super::*;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditSnapshot {
    text: Arc<str>,
    selection: TextSelection,
}

impl TextEditBuffer {
    pub(super) fn snapshot(&self) -> EditSnapshot {
        // Adjacent edits normally have the same text at their shared boundary,
        // but a cursor move may give that text a different directed selection.
        // Share only immutable text, never the selection or mutable document.
        let text = self
            .undo
            .last()
            .map(|(_, after)| after)
            .into_iter()
            .chain(self.redo.last().map(|(before, _)| before))
            .find(|snapshot| snapshot.text.as_ref() == self.text)
            .map(|snapshot| Arc::clone(&snapshot.text))
            .unwrap_or_else(|| Arc::from(self.text.as_str()));
        EditSnapshot {
            text,
            selection: self.selection,
        }
    }

    pub(super) fn restore(&mut self, snapshot: EditSnapshot) {
        self.text = snapshot.text.to_string();
        self.selection = snapshot.selection;
        self.composition = None;
    }

    // All user mutations enter here; nested helpers belong to the same edit.
    pub(super) fn edit<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, TextEditError>,
    ) -> Result<T, TextEditError> {
        let outer = self.edit_depth == 0;
        let before = outer.then(|| {
            self.composition_before
                .clone()
                .unwrap_or_else(|| self.snapshot())
        });
        self.edit_depth += 1;
        let result = f(self);
        self.edit_depth -= 1;
        if outer && result.is_ok() && self.composition.is_none() {
            self.composition_before = None;
            let before = before.unwrap();
            if before.text.as_ref() != self.text {
                self.redo.clear();
                self.undo.push((before, self.snapshot()));
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
            }
        }
        result
    }

    pub fn undo(&mut self) -> Result<TextEditOutcome, TextEditError> {
        if self.composition.is_some() {
            self.cancel_composition()?;
            return Ok(changed());
        }
        let Some(entry) = self.undo.pop() else {
            return Ok(TextEditOutcome::default());
        };
        self.restore(entry.0.clone());
        self.redo.push(entry);
        Ok(changed())
    }

    pub fn redo(&mut self) -> Result<TextEditOutcome, TextEditError> {
        if self.composition.is_some() {
            return Ok(TextEditOutcome::default());
        }
        let Some(entry) = self.redo.pop() else {
            return Ok(TextEditOutcome::default());
        };
        self.restore(entry.1.clone());
        self.undo.push(entry);
        Ok(changed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_edits_share_text_allocations() {
        let mut buffer = TextEditBuffer::with_text("x".repeat(64 * 1024));
        for _ in 0..100 {
            buffer.insert_text("a").unwrap();
        }
        let mut allocations = std::collections::HashMap::new();
        for (before, after) in &buffer.undo {
            for snapshot in [before, after] {
                allocations.insert(snapshot.text.as_ptr(), snapshot.text.len());
            }
        }
        let owned_bytes: usize = allocations.values().sum();
        eprintln!(
            "history unique texts={} owned text bytes={owned_bytes}",
            allocations.len()
        );
        assert_eq!(allocations.len(), 101);
        assert_eq!(owned_bytes, (0..=100).map(|n| 64 * 1024 + n).sum::<usize>());
    }

    #[test]
    fn shared_text_keeps_selection_and_cloned_documents_independent() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TextEditBuffer>();
        let mut buffer = TextEditBuffer::with_text("abc");
        buffer.insert_text("d").unwrap();
        buffer.set_selection(TextSelection::new(3, 1)).unwrap();
        buffer.insert_text("X").unwrap();
        assert!(Arc::ptr_eq(&buffer.undo[0].1.text, &buffer.undo[1].0.text));
        assert_eq!(buffer.undo[0].1.selection, TextSelection::collapsed(4));
        assert_eq!(buffer.undo[1].0.selection, TextSelection::new(3, 1));
        let mut clone = buffer.clone();
        clone.undo().unwrap();
        assert_eq!(clone.text(), "abcd");
        assert_eq!(clone.selection(), TextSelection::new(3, 1));
        assert_eq!(buffer.text(), "aXd");
        clone.undo().unwrap();
        clone.redo().unwrap();
        assert_eq!(clone.selection(), TextSelection::collapsed(4));
        clone.redo().unwrap();
        assert_eq!(clone.text(), "aXd");
        assert_eq!(clone.selection(), TextSelection::collapsed(2));
    }

    #[test]
    fn composition_shares_start_text_without_mutating_history() {
        let mut buffer = TextEditBuffer::with_text("原");
        buffer.insert_text("文").unwrap();
        let committed = Arc::clone(&buffer.undo.last().unwrap().1.text);
        buffer.begin_composition("候选").unwrap();
        assert!(Arc::ptr_eq(
            &committed,
            &buffer.composition_before.as_ref().unwrap().text
        ));
        buffer.cancel_composition().unwrap();
        assert_eq!(buffer.text(), "原文");
        assert_eq!(committed.as_ref(), "原文");
        assert_eq!(buffer.undo.len(), 1);
        buffer.begin_composition("候选").unwrap();
        buffer.commit_composition("新").unwrap();
        assert_eq!(buffer.text(), "原文新");
        assert!(Arc::ptr_eq(&committed, &buffer.undo.last().unwrap().0.text));
        buffer.undo().unwrap();
        assert_eq!(buffer.text(), "原文");
        buffer.redo().unwrap();
        assert_eq!(buffer.text(), "原文新");
    }

    #[test]
    fn eviction_and_redo_branch_release_unreferenced_text() {
        let mut buffer = TextEditBuffer::with_text("start");
        buffer.insert_text("a").unwrap();
        let first = Arc::downgrade(&buffer.undo[0].0.text);
        for _ in 0..100 {
            buffer.insert_text("b").unwrap();
        }
        assert!(first.upgrade().is_none());
        let discarded = Arc::downgrade(&buffer.undo.last().unwrap().1.text);
        buffer.undo().unwrap();
        assert!(discarded.upgrade().is_some());
        let boundary = Arc::clone(&buffer.redo.last().unwrap().0.text);
        buffer.insert_text("c").unwrap();
        assert!(buffer.redo.is_empty());
        assert!(discarded.upgrade().is_none());
        assert!(Arc::ptr_eq(&boundary, &buffer.undo.last().unwrap().0.text));
        let current = Arc::downgrade(&buffer.undo.last().unwrap().1.text);
        buffer = TextEditBuffer::new();
        assert!(current.upgrade().is_none());
        assert!(buffer.undo.is_empty());
    }

    #[test]
    fn bounded_history_releases_evicted_snapshots_and_noops_keep_redo() {
        let mut buffer = TextEditBuffer::new();
        for _ in 0..105 {
            buffer.insert_text("a").unwrap();
        }
        assert_eq!(buffer.undo.len(), 100);
        assert_eq!(buffer.undo.first().unwrap().0.text.as_ref(), "aaaaa");
        buffer.undo().unwrap();
        buffer.insert_text("").unwrap();
        assert_eq!(buffer.redo.len(), 1);
        buffer.redo().unwrap();
        assert_eq!(buffer.text().len(), 105);
        // Logical snapshots remain independent. Shared text is freed when its
        // last owner disappears; entry count still does not bound text bytes.
        let bytes: usize = buffer
            .undo
            .iter()
            .map(|(a, b)| a.text.len() + b.text.len())
            .sum();
        assert_eq!(bytes, (5..105).map(|n| n + n + 1).sum::<usize>());
        buffer = TextEditBuffer::with_text("reset");
        assert!(buffer.undo.is_empty() && buffer.redo.is_empty());
    }
}
