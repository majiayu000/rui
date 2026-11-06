# Local text Undo and Redo

Status: design only; not implemented by the runtime-correctness PR. Tracking: [#288](https://github.com/majiayu000/rui/issues/288). Background: [source comparison](../research/rui-gpui-bui-2026-11-07.md).

## Scope

One editing state owns its history, covering Input and TextArea through the existing text-editing core. No global app history, transaction service, new persistence layer, or copied third-party text buffer. Preserve UTF-16 native interfaces and the existing composition state machine.

## Required semantics

- An entry stores the text and selection before/after an edit; UTF-8 storage must not split Unicode scalars when restoring native UTF-16 ranges.
- Each ordinary insert/delete or paste is one transaction initially. Do not invent time-based typing merge heuristics.
- At composition start, retain the pre-composition text and selection. Updates to marked text are provisional; commit yields one history entry; composition cancellation yields none and restores the original state.
- While composition is active, Undo cancels the provisional composition first without consuming an older committed history entry. Redo does nothing until composition ends. The native owner must unmark consistently with that state transition.
- Undo restores text and selection; Redo reapplies both. Editing after Undo clears the redo branch. Empty/no-op changes do not create entries.
- Programmatic set-value resets local history; it is not silently recorded as a user edit. This distinction must be implemented at the authoritative edit boundary.
- Histories stay with the editing state, not global focus. Changing focus never transfers one field's history to another.
- Initially retain at most 100 committed edits per field, dropping the oldest when full. This bounds count, not total bytes; large-text memory costs must be stated and measured before broader editor claims. Do not expose a new config surface in the initial implementation.

## Integration

Route Undo/Redo through the existing action/text command path to the current text owner. Keyboard bindings should not steal shortcuts from an explicit application binding. Native composition updates, keyboard typing and paste must converge on one history mutation path, without recording the same operation at multiple layers.

## Acceptance

- Type, replace a selection, paste, delete, Undo and Redo; assert text and exact selection each time.
- Include Chinese, emoji/surrogate pairs, combining characters, and multi-line edits.
- Multiple marked-text updates plus commit produce one undo; cancel produces none.
- Undo during composition restores pre-composition text, then a second Undo affects prior committed history.
- New input after Undo clears Redo; programmatic replacement clears both branches.
- Two focused inputs retain separate histories; detach/remount behavior follows existing state ownership.
- Exceed the history limit and verify retained entries and resource release.
- Headless tests establish state semantics; a real native IME session separately verifies marked text and candidate behavior.

## Decision

Adopt local editing transactions, not BUI's cross-widget history or interaction-count merge rules. History size is a concrete initial policy, not a general undo framework. Implementation remains pending; the design must not be presented as an existing API.
