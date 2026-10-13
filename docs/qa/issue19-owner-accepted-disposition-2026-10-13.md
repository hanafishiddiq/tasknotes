# Issue19 — current ordinary-app symptom no longer reproducible (owner report)

**Date:** 2026-10-13. **Basis:** explicit owner report and existing native QA evidence; no new product code or performance experiment.

## What is now established

- The owner says **ordinary Tasknotes is smooth**; earlier native scrolling felt 10–20 FPS on a prior build, which was subjective.
- The separate diagnostic application **Tasknotes Issue19 Hover A B QA** remained laggy. The user explicitly directed us to stop working on lag that occurs only in that test tool.
- The actual ordinary and QA binaries embedded the same React/CSS assets at the last detailed audit, but the QA added a diagnostic script and used a different launch/process context. A build-path collision was eliminated in Draft PR #12, without altering ordinary Tasknotes source or data. A matched physical display-rate root cause was never established.
- Existing native create/edit/restart and progress data acceptance occurred on isolated synthetic SQLite in the Tasknotes program. This does **not** independently certify release-scale performance.

## Decision

The local bug's **triage status is `wontfix`**, meaning deliberately **do not continue a QA-only performance investigation** when the real product symptom is no longer reproduced. The separate `Implementation` field marks the **current ordinary-app issue closed by observation**, not fixed by an identified patch. Historical measurements and in-progress, unstaged diagnostic source in the old worktree remain untouched. Draft PR #5–#12 are **not** merged or promoted on the strength of this disposition.

This distinction follows `docs/agents/triage-labels.md`: `wontfix` is a triage role, not a claim that a feature ticket was implemented. No secret, screenshot, personal database or raw Safari recording is included.

## Future gates, not waived

- Ticket 17 must still measure search/scroll/memory on **50,000 synthetic Tasks plus 10,000 synthetic Notes**, including scrolling near 60 FPS when its dependencies exist.
- Ticket 18 still requires macOS integration, backup/restore, privacy, release and independent review checks; none is passed by closing the earlier single-Task symptom.
- If actual ordinary Tasknotes scrolling lag recurs, **reopen/re-triage** with a new owner-observed reproduction rather than extrapolating from any QA-only rAF value.

## Scope verification

This handoff changes only local issue tracker and documentation, never `src/`, `src-tauri/`, core schema or shipping bundle. The original dirty diagnostic worktree and its untracked notes are deliberately preserved. Main and earlier Draft PRs remain unchanged. Independent code review for Tickets 01–03 remains a separate outstanding gate.
