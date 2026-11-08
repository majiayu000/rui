# Task context and progress boundaries

Status: implementation specification, 2026-11-07. Baseline: 1311a1d. Issues: [#284](https://github.com/majiayu000/rui/issues/284), [#285](https://github.com/majiayu000/rui/issues/285).

## Problem and observed evidence

A saved AsyncApp belonging to A can mutate B when invoked during B's task poll. A future panic skips the manual restoration of POLLING_APP; catch-and-continue callers can then call update outside polling. Separately, an inline background Future that always wakes itself never lets run_inline_tasks return, bypassing the existing MAX_PARK_ROUNDS guard. BackgroundTask cancellation may retain Pending futures through external wakers; confirm this case with a full-runtime regression first.

## Required behavior

1. AsyncApp updates are valid only while its owning AppContext is being polled, and never inside another update. Invalid use must fail before running the callback or mutating either app.
2. Bind ownership to an existing stable queue/executor identity, not the movable AppContext address; do not create a global ID registry. Validate once at the update boundary.
3. Poll context is scoped and restored on normal completion and unwinding. Preserve panic propagation rather than swallowing it or retrying tasks. Same-app work remains usable after a caller catches a panic.
4. An inline drain processes the ready batch present at entry. Wakeups during that batch remain queued for subsequent rounds. Keep the existing round limit and FIFO policy; do not introduce a configurable time budget.
5. Dropping a non-detached BackgroundTask requests cancellation and ensures executor progress can dispose of the future even if an external waker remains held. Drop is non-blocking; a currently running synchronous poll cannot be forcibly interrupted. Detached tasks remain live.

## Regression acceptance

- Red then green: A-context inside B poll does not run its closure, A/B values stay unchanged; same-owner update succeeds, including after AppContext moves.
- Red then green: after catch_unwind around a panicking foreground task, update outside poll is rejected; a later valid task succeeds; nested update remains rejected.
- Red then green: one inline batch polls a self-waking task once; finite self-wakers complete; an endless self-waker reaches the existing round-limit panic. Never run an unbounded baseline test without an external timeout.
- Full-runtime retained-waker/drop-counter regression for cancellation: cancel, make executor progress, observe captured resource destruction without invoking the external waker. Preserve drop-before-first-poll and detach semantics.
- Run focused executor and spawn suites, then the relevant integration/replay checks. Describe native worker-thread verification separately from headless tests.

## Scope and non-goals

Implementation stays in the existing executor and its regression tests. No new scheduler, supervision policy, retry layer, public configuration or compatibility API. No GUI/IME changes. Use original RUI implementation, not copied GPUI/BUI code. Performance claims and platform support are outside this change.

## Decision

Adopt the existing ownership and scheduling model with correct boundary handling. Reject a runtime rewrite. Completion requires observed regression behavior, not only successful compilation.
