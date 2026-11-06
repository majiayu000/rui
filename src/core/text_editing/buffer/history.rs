use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditSnapshot {
    text: String,
    selection: TextSelection,
}

impl TextEditBuffer {
    pub(super) fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            text: self.text.clone(),
            selection: self.selection,
        }
    }

    pub(super) fn restore(&mut self, snapshot: EditSnapshot) {
        self.text = snapshot.text;
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
            if before.text != self.text {
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
    fn bounded_history_releases_evicted_snapshots_and_noops_keep_redo() {
        let mut buffer = TextEditBuffer::new();
        for _ in 0..105 {
            buffer.insert_text("a").unwrap();
        }
        assert_eq!(buffer.undo.len(), 100);
        assert_eq!(buffer.undo.first().unwrap().0.text, "aaaaa");
        buffer.undo().unwrap();
        buffer.insert_text("").unwrap();
        assert_eq!(buffer.redo.len(), 1);
        buffer.redo().unwrap();
        assert_eq!(buffer.text().len(), 105);
        // Both snapshots are owned; dropping an entry drops its Strings. The
        // initial policy bounds entries rather than aggregate text bytes.
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
