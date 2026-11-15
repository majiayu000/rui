# Variable-height list: bounded initial contract

Implementation: [PR #292](https://github.com/majiayu000/rui/pull/292).

Status: implemented in the #287 feature branch; validation commands and boundaries below. Tracking: [#287](https://github.com/majiayu000/rui/issues/287). Baseline: 1311a1d.

## User task

Read a chat/log with wrapped text and late-arriving content without losing the current item when history is prepended, rows change height, or the window narrows. Keep existing equal-height VirtualList unchanged.

## Initial surface

A VariableList element and persistent handle at the existing element layer. API:

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

## Implementation and decision

Adopt this explicit-viewport element and the shared Taffy/text measurement path.
Reject a separate renderer, scheduler, data source abstraction, or adaptive estimate API.

`VariableListHandle::anchor()` exposes the current `(ElementId, pixel_offset)` after
layout. `offset()` and `estimated_total_height()` are estimates because unseen rows
use an internal 40px height. `scroll_to_item(id)` aligns the item to the top;
`scroll_to(px)` navigates estimated geometry. A missing jump ID is ignored.
Invalidations and handle scrolling take effect at the next layout; the caller must
request a frame when changing these outside an event that already requests one.

Rows are measured at the specified width and mounted as absolute children of the
fixed viewport. A fixed 80px neighborhood is built on either side. Metadata scans
and ID validation are O(N); there is no claim of O(log N) edits or a measured
100k-frame latency budget. Positive row height is a contract; zero-height rows
must be removed from the sequence. No scrollbar or tail-follow mode is included.
An item jump near the end may leave space below the last row, because item alignment
and surviving-anchor preservation take precedence over bottom alignment. The same
anchor rule applies when a resize or content edit shortens the remaining tail.
Ordinary pixel/wheel scrolling backfills the measured end to avoid blank tail space
and an estimated-height correction jumping the reader backward.

The presenter passes its actual focus owner into layout, including keyboard and
programmatic focus. Reconciliation transfers row instances only; new list props
and handle stay authoritative. A surviving focused row stays mounted offscreen,
so keyboard/IME state survives scrolling and view rebuilding. Explicitly invalidating that
row cancels its active composition, clears focus, and rebuilds its new content
immediately. Other visible invalidations also rebuild immediately. Removing a focused row cancels its active composition on
the old logical editor before dropping it; the presenter clears the absent focus
owner. Replacing a handle on the same reconciled list cancels old composition
and rebuilds all rows. Root-list identity replacement also performs that cleanup.
Keep a nested list and its ancestors structurally present with stable container
IDs during editing: removing the whole list/ancestor, or replacing a keyed nested
list identity, bypasses existing reconciliation and is not covered by this
row-virtualization contract. Whole-subtree unmount cancellation requires a
separate runtime lifecycle change. A custom editor that refuses to cancel its active
composition fails explicitly instead of silently discarding it. The list owns no
native IME session; actual candidate-window behavior still needs platform testing.

Validation: `RUI_VERIFY_REPLAY=1 cargo test --test variable_list`; neighboring
fixed-height list and retained-layout tests; `cargo check --example variable_list`.
The executable example is `cargo run --example variable_list`.
