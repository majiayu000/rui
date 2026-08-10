use std::collections::HashSet;

#[derive(Debug, Default)]
pub(super) struct SuppressedKeyUps {
    key_codes: HashSet<u16>,
}

impl SuppressedKeyUps {
    pub(super) fn record_key_down(&mut self, key_code: u16, consumed: bool) {
        if consumed {
            self.key_codes.insert(key_code);
        } else {
            self.key_codes.remove(&key_code);
        }
    }

    pub(super) fn should_emit_key_up(&mut self, key_code: u16) -> bool {
        !self.key_codes.remove(&key_code)
    }

    pub(super) fn forget_key_up(&mut self, key_code: u16) {
        self.key_codes.remove(&key_code);
    }

    pub(super) fn clear(&mut self) {
        self.key_codes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::SuppressedKeyUps;

    #[test]
    fn suppresses_only_the_matching_consumed_key_pair() {
        let mut suppressed = SuppressedKeyUps::default();
        suppressed.record_key_down(36, true);

        assert!(suppressed.should_emit_key_up(0));
        assert!(!suppressed.should_emit_key_up(36));
        assert!(suppressed.should_emit_key_up(36));
    }

    #[test]
    fn new_unconsumed_down_replaces_stale_suppression() {
        let mut suppressed = SuppressedKeyUps::default();
        suppressed.record_key_down(36, true);
        suppressed.record_key_down(36, false);

        assert!(suppressed.should_emit_key_up(36));
    }

    #[test]
    fn focus_loss_and_foreign_release_clear_stale_suppression() {
        let mut suppressed = SuppressedKeyUps::default();
        suppressed.record_key_down(36, true);
        suppressed.forget_key_up(36);
        assert!(suppressed.should_emit_key_up(36));

        suppressed.record_key_down(36, true);
        suppressed.clear();
        assert!(suppressed.should_emit_key_up(36));
    }
}
