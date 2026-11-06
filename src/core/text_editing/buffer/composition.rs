use super::*;

impl TextEditBuffer {
    pub fn commit_composition(&mut self, text: &str) -> Result<(), TextEditError> {
        self.edit(|this| {
            this.ensure_text_allowed(text)?;
            if let Some(composition) = this.composition.take() {
                this.replace_range_internal(composition.replacement_range(), text)?;
                this.selection =
                    TextSelection::collapsed(composition.replacement_range().start() + text.len());
            } else {
                let range = this.selected_range();
                this.replace_range_internal(range, text)?;
                this.selection = TextSelection::collapsed(range.start() + text.len());
            }
            Ok(())
        })
    }

    pub fn commit_composition_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<(), TextEditError> {
        self.edit(|this| {
            this.ensure_text_allowed(text)?;
            let replacement_range = replacement_range.to_text_range(&this.text)?;
            this.replace_range_internal(replacement_range, text)?;
            this.composition = None;
            this.selection = TextSelection::collapsed(replacement_range.start() + text.len());
            Ok(())
        })
    }

    pub fn commit_current_composition(&mut self) -> Result<(), TextEditError> {
        self.edit(|this| {
            let composition = this
                .composition
                .take()
                .ok_or(TextEditError::CompositionMissing)?;
            this.selection = TextSelection::collapsed(composition.replacement_range().end());
            Ok(())
        })
    }
}

impl TextEditBuffer {
    pub fn begin_composition(&mut self, text: &str) -> Result<(), TextEditError> {
        if self.composition.is_some() {
            return Err(TextEditError::CompositionActive);
        }
        self.ensure_text_allowed(text)?;
        self.begin_composition_in_range(text, self.selected_range())
    }

    pub fn begin_composition_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<(), TextEditError> {
        if self.composition.is_some() {
            return Err(TextEditError::CompositionActive);
        }
        self.ensure_text_allowed(text)?;
        let replacement_range = replacement_range.to_text_range(&self.text)?;
        self.begin_composition_in_range(text, replacement_range)
    }

    fn begin_composition_in_range(
        &mut self,
        text: &str,
        original_range: TextRange,
    ) -> Result<(), TextEditError> {
        let before = self.snapshot();
        let original_text = self.text[original_range.start()..original_range.end()].to_string();
        self.replace_range_internal(original_range, text)?;
        self.composition_before = Some(before);
        let range = TextRange::ordered(original_range.start(), original_range.start() + text.len());
        self.selection = TextSelection::collapsed(range.end());
        self.composition = Some(TextComposition::new(
            range,
            text.to_string(),
            original_range,
            original_text,
        ));
        Ok(())
    }

    pub fn update_composition(&mut self, text: &str) -> Result<(), TextEditError> {
        self.ensure_text_allowed(text)?;
        let composition = self
            .composition
            .clone()
            .ok_or(TextEditError::CompositionMissing)?;
        self.replace_range_internal(composition.replacement_range(), text)?;
        let range = TextRange::ordered(
            composition.replacement_range().start(),
            composition.replacement_range().start() + text.len(),
        );
        self.selection = TextSelection::collapsed(range.end());
        self.composition = Some(TextComposition::new(
            range,
            text.to_string(),
            composition.original_replacement_range(),
            composition.original_text().to_string(),
        ));
        Ok(())
    }

    pub fn update_composition_replacing_utf16(
        &mut self,
        text: &str,
        replacement_range: Utf16TextRange,
    ) -> Result<(), TextEditError> {
        self.ensure_text_allowed(text)?;
        let replacement_range = replacement_range.to_text_range(&self.text)?;
        let composition = self
            .composition
            .clone()
            .ok_or(TextEditError::CompositionMissing)?;
        let (original_range, original_text) =
            if replacement_range == composition.replacement_range() {
                (
                    composition.original_replacement_range(),
                    composition.original_text().to_string(),
                )
            } else {
                (
                    replacement_range,
                    self.text[replacement_range.start()..replacement_range.end()].to_string(),
                )
            };
        self.replace_range_internal(replacement_range, text)?;
        let range = TextRange::ordered(
            replacement_range.start(),
            replacement_range.start() + text.len(),
        );
        self.selection = TextSelection::collapsed(range.end());
        self.composition = Some(TextComposition::new(
            range,
            text.to_string(),
            original_range,
            original_text,
        ));
        Ok(())
    }
}
