# Native interaction acceptance

Run `cargo run --example interaction_acceptance` on macOS. The example keeps
all edits in memory; Save confirms an edit, not a filesystem write.

1. From a fresh window, Tab moves through Filter, Draft, Save, Revert draft
   and Open independent editor.
   Disabled is skipped. The closed confirmation's buttons must not be reached.
2. Shift-Tab reverses that order and wraps from Filter to Open independent editor.
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

## Multi-window focus isolation (manual check pending)

Activate Open independent editor to create a second editor with its own state.
The two editors intentionally reuse element IDs, as normal instances of a view do.

1. Put different text in each Draft. Switch between the windows and use Tab and
   Shift-Tab: only the active window's focus should move, including wraparound.
2. Leave Draft focused in one window and Save focused in the other. Type into
   Draft, then switch windows and press Enter on Save. Only that window should
   open its confirmation; Confirm, Cancel and Revert must affect only its draft,
   saved value and status.
3. Close one window while its Draft is focused. The remaining window must still
   accept text, Tab/Shift-Tab and Save/Confirm without changing its own saved data
   unexpectedly. Repeat with the other window closed first.

`cargo test --test multi_window` exercises shared-AppContext keyboard traversal,
text ownership, Enter activation and continued editing after window close.
These headless assertions do not verify AppKit key-window switching, native IME
candidate composition or VoiceOver speech. This new manual sequence has not yet
been run on the native example.

The default keymap currently activates buttons with Enter, not Space; a headless
Space probe did not activate the button. Space support is outside this change.
