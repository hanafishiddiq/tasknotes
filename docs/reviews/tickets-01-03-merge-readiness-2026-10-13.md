# Tasknotes v1 — merge and handoff gates (NOT an execution authorization)

> **Archived pre-merge HOLD (superseded):** This document predates the source-only integration of PR #1 → #2 → #3. Those three feature PRs are now merged; refer to the [post-merge evidence](../qa/tickets-01-03-postmerge-2026-10-13.md). Its precautions about QA PRs and migration still apply.

Date 2026-10-13. Decision state: **HOLD** pending owner authorization. No merge, release, installation, personal SQLite mutation or deployment was performed.

## Preconditions verified

- Ticket 01–03 source implementation and prior real native synthetic acceptance exist; 39 React/Rust tests and typecheck/build/fmt/clippy passed in a clean worktree. Six isolated external-AI Standards/Spec reviews and a factual cross-check are archived in [the review record](tickets-01-03-independent-2026-10-13.md). No currently proven blocking implementation defect was found.
- GitHub PRs #1, #2, #3 are **Draft**. The earlier Copilot bot submissions say its review quota was exhausted, not `APPROVED`. We have a legitimate *independent AI reviewer process*, **not** a human GitHub review or approval.
- Main remains `e4c728c2d175404a6de79ad2bc207eb40a9e0874` at the last read. Revalidate the live remote SHA and CI before any future write; never use this historical value as a blind merge lease.
- The running ordinary Tasknotes app is owner-accepted as smooth. Issue19 QA lag is a stopped *diagnostic-only pursuit*. Do not merge experimental Issue19 PR #5–#12 just to gain a feature or record a bug disposition.

## Safe action order, only AFTER an explicit owner merge instruction

1. Confirm whether **independent AI** code-review evidence is sufficient for the project's gate, or an authenticated human `APPROVED` review is additionally required. Collect and resolve any real defects. Revalidate each PR head/base, approvals, CI, diff, and current branch head; confirm no unreviewed changes slipped in.
2. Review PR **#1** relative to main, then merge only if specifically authorized. Review PR **#2** relative to Ticket 01, and retarget its base to the updated main **only after** PR #1 is integrated. Review PR **#3** relative to Ticket 02, then retarget/integrate after #2. Avoid merging accumulated stacked changes out of order, duplicate cherry-picks or force-pushes.
3. Separately decide whether to integrate **PR #4**: a debug-only native WKWebView/WebDriver test harness and associated native acceptance evidence. Its release-mode feature is compile-refused by source. It is not the abandoned Issue19 lag-investigation chain. If PR #4 is intentionally NOT merged, cherry-pick only documentation from **PR #14** to an authorized feature-derived branch (don't pull its parent dependency by accident).
4. **PR #13** is a *documentation disposition* currently stacked on **PR #12 (abandoned Issue19 QA chain)**. Its text is valid, but **merging that stack as-is would also drag unnecessary diagnostic code**. Port/cherry-pick **only** its tracker/README disposition change to the accepted product/integration branch, preserving original historical measurements, after authorized review. Never merge #13's QA parents simply to carry the closure note.
5. Before launching an upgraded app against a real Tasknotes SQLite database, take and verify a **recoverable backup** and record a versioned migration+rollback procedure: Ticket 01 schema v1, Ticket 02 v2, Ticket 03 v3 are forward-only and older builds reject newer schemas. A GitHub source merge is not itself a data migration, but release/installation may be.
6. Only then, in tracker order, mark Ticket 01, then 02, then 03 `Implementation: completed` when their actual criteria, independent-review policy, dependency and merge gates are satisfied; recompute the frontier: Tickets **04 and 06** become next possible feature slices, **not yet started**. Each needs its own implementation instruction and test seams; Ticket 17/18 still require independent work.

## Safeguards

- Existing public-review outputs are opinions, not source-of-truth overrides. Correct false findings against frozen diff/code/test evidence before patching.
- No personal Task/Note data may enter Git; no secrets, screenshots, Safari recordings, system permissions, or QA startup hacks.
- Preserve the existing original dirty Issue19 worktree and `MEMORY.md`. Any code fix must use a separate correct parent branch and an agreed public test seam with red→green proof.
- The current operation is **independent AI code review and preparation only**, not user approval to merge.
