# Native interaction acceptance

Run `cargo run --example interaction_acceptance` on macOS. The example keeps
all edits in memory; Save confirms an edit, not a filesystem write.

1. From a fresh window, Tab moves through Filter, Draft, Save and Revert draft.
   Disabled is skipped. The closed confirmation's buttons must not be reached.
2. Shift-Tab reverses that order and wraps from Filter to Revert draft.
3. Type a filter: the Note row disappears for a non-matching query and returns
   when cleared. Edit Draft, activate Save, then Cancel. Draft stays unchanged
   and Saved stays at its previous value.
4. Save and Confirm. Saved must equal Draft. Edit again and Revert draft: Draft
   must equal Saved. Repeat with Chinese and emoji pasted from the clipboard.
5. Inspect the native accessibility tree: its focused element must follow the
   actual keyboard owner, including buttons. Removing a focused confirmation
   button must not leave that removed node exposed as the focused element.

Separate checks, not implied by the steps above:

- Use an installed Chinese IME to compose pinyin, choose a candidate, cancel
  composition and change focus while composing. Unicode paste is not this test.
- Enable VoiceOver and verify spoken labels, values and focus changes. Reading
  the accessibility tree is useful evidence but does not verify speech output.
- Native file-save sheets and filesystem persistence are outside this example.

## Observed on 2026-11-07

Before the fixes, the native accessibility client repeatedly timed out while
opening the app; after completing AppKit launch, it could enumerate controls.
Focus was initially reported as the window despite text going into an input;
resolving the content view's focused accessibility descendant corrected this.
Shift-Tab initially did nothing because AppKit's U+0019 mapped to Unknown(25);
normalizing that character to Tab restored reverse navigation.

Actual native input verified forward/reverse focus, wraparound, skipping disabled
and hidden controls, cancel, save, revert, and Chinese/emoji paste and save.
IME candidate composition and VoiceOver speech were not verified.
