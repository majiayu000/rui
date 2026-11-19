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

## Multi-window focus isolation

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

A window that loses key status shows no focused element, then focuses again
what it had when it becomes key. Focus is not given back when app code moved or
cleared it meanwhile, or the element was removed, disabled or is behind a modal
that opened.

`cargo test --test multi_window` exercises shared-AppContext keyboard traversal,
text ownership, Enter activation, key-window switching
(`HeadlessApp::make_key_window`) and continued editing after window close;
`cargo test --test focus` covers a single window losing and regaining key
status. These headless assertions do not verify native IME candidate
composition or VoiceOver speech.

### Observed on 2026-11-19

A driver built from this example (the Editor verbatim) was launched as an app
bundle through LaunchServices so AppKit made it active. It switched key status
with `makeKeyAndOrderFront:` (the window server reported `isKeyWindow` true for
each switch), posted key `NSEvent`s to the key window and read each window's
focused node from its NSAccessibility tree. With the fix all 16 checks passed:

- Six Tabs wrapped from Open independent editor to Filter; Shift-Tab wrapped
  back, then reached Revert draft.
- Enter on Open independent editor opened the second editor as key; returning
  to the first window focused Open independent editor again.
- Steps 1 and 2: Draft focused in the first window, Save in the second. The
  inactive window exposed no focused element each time. Switching back focused
  Draft and typing went into it; switching to the second window focused Save,
  and Enter opened only that window's confirmation. Confirm saved only its draft.
- Step 3: closing the second window with its Draft focused left the first
  window's Draft focused; it took text and Tab.

The same driver on the previous main (a646536) failed 8 checks: after every
switch back nothing was focused, so Enter on Save did nothing and Tab started
over from Filter. Typed letters still reached the first window's Draft while
nothing owned focus.

## Focused button Space activation

An enabled, editable button activates on the initial unmodified Space key-down.
Holding Space does not activate again on repeat key-downs; key-up does not
activate. Enter retains its existing action binding. Explicit app key bindings
still run before the button's raw-key fallback.

To check manually in the example, focus Save and press Space: the confirmation
opens once. Release Space and verify it does not confirm the newly focused
button. Cancel, focus Draft and type spaces; they must edit Draft without opening
confirmation. Tab past Disabled and repeat Save with Enter.

A two-window native probe on macOS posted actual AppKit `NSEvent`s to named
windows. The focused button count changed 0 → 1 on Space down, stayed 1 on repeat
and up, and became 2 after Enter down/up. In the input, Space and repeat inserted
two spaces without clicking either button. Tab returned from the input to the
button, skipped Disabled, and Space activated it again. Space in the second
window changed only that window's count. The probe initially failed at Space
down because AppKit interpreted text even without a text-input snapshot; only
text owners now enter `interpretKeyEvents`.

This scripted native check is separate from physical keyboard testing, live IME
candidate selection, and VoiceOver speech; those were not verified for this change.
