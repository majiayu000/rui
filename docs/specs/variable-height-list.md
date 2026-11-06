# Variable-height list: bounded initial contract

Status: design only; not implemented by the runtime-correctness PR. Tracking: [#287](https://github.com/majiayu000/rui/issues/287). Baseline: 1311a1d. Background: [source comparison](../research/rui-gpui-bui-2026-11-07.md).

## User task

Read a chat/log with wrapped text and late-arriving content without losing the current item when history is prepended, rows change height, or the window narrows. Keep existing equal-height VirtualList unchanged.

## Initial surface

A VariableList element and persistent handle at the existing element layer. Proposed API (not currently available):

```text
variable_list(&handle, stable_item_ids, |id| render_row(id)).w(width).h(height)
handle.invalidate(item_id)
handle.scroll_to_item(item_id)
```

IDs are unique stable ElementIds, not positions. Explicit finite positive viewport dimensions are required initially. Changing row content requires invalidate(id); changing width invalidates measured heights. No w_full promise until final parent constraints can be used correctly. No generic key trait, paging source, backend adapter, estimator plugin, skeleton mode, or tail-follow option.

## State and reading position

The handle keeps item order, measured heights keyed by item ID, measurement width, and anchor (ID, offset within item). Unknown heights use an internal estimate; reported total height is approximate until measured.

- Prepend/append retains the same surviving anchor and its intra-item pixel offset.
- Height changes above the anchor do not accumulate into visible drift.
- On width change preserve item identity and clamp intra-item offset after measurement. This does not promise the same text glyph remains at the same screen position.
- Deleting the anchor selects the first surviving successor in old order, then a predecessor, otherwise empty. Reset intra-item offset for a replacement anchor.
- Scroll-to-item resolves to that ID without constructing intervening rows.
- Appending does not force the reader to the bottom in this version.

## Layout integration

Current LayoutContext::compute_layout consumes the context and ends the retained pass. Do not call it per row. Establish a private borrowed subtree-measurement helper with known width; reuse the existing text-measure callback, inherited typography and text cache without ending the pass. Final whole-tree layout remains the authority for node reclamation.

Build candidates around the anchor, measure and extend until the viewport plus fixed overscan is covered. Reconcile by stable item identity; invalidated visible content must actually rebuild. Paint, hit testing and text geometry use the same row bounds and clipping. Never fix measurements by mutating data in paint or assuming last frame's width is current.

Positive row heights are required to bound visible row construction. Invisible items should be removed from the ID sequence. Validate the public contract once; do not duplicate it through each layer. Metadata reconciliation may initially be O(N); expensive element building/measurement must remain proportional to the visible neighborhood. Do not advertise O(log N) edit costs or million-row frame performance without measurement.

## Acceptance

1. A 100k-item fixture with mixed 20/80px heights and a 300px viewport builds/measures only nearby rows; use actual builder counters on first frame, scroll and resize.
2. Start inside ID 5000, prepend 100 items: same surviving ID and offset within one pixel; clicking still selects the displayed ID.
3. Append while reading, increase a row above the viewport, and invalidate a visible row: preserve anchor and display fresh content.
4. Narrow wrapped text: use the new width, fill the viewport and preserve anchor ID; clamp offsets to new geometry.
5. Delete anchor, clear data, and jump far: deterministic fallback and no out-of-bounds or building all intervening rows.
6. Clipped parts cannot receive pointer input. Retained/fresh scene and layout results agree after edits and resize.
7. Focused editable-row policy is a prerequisite to claiming editing support: either retain the focused row explicitly or cancel its composition before removal. Never silently discard marked text. This is not waived by list geometry tests.

## Decision / implementation gate

Adopt anchor semantics and one shared layout/paint path. Reject a separate list platform. Start implementation only with the subtree-measurement contract and focus policy reviewed; they are actual architecture work, not a cosmetic new builder. This specification does not claim a finished feature.
