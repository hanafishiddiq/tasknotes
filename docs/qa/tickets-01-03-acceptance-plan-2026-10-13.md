# Tasknotes v1 Tickets 01–03 — acceptance reconciliation plan

**Date:** 2026-10-13. **Review fixed point:** Ticket 03 `2937c9a0c8abb1beed5e799a3cc96c07aae18053`. **Documentation branch base:** Draft PR #4 (`test/native-wdio-e2e`) so native QA harness and accepted report are present in the same source tree; not a merge into main.

## Definition

Tickets 01–03 have implemented native UI/SQLite/Rust functionality. The recorded manual native create/edit/reopen and automated two-process WDIO acceptance **already passed** on 2026-10-12. Historic self-review text still calls these paths unverified because it predates the later evidence. The remaining independent reviewer and merge-approval gates must not be misrepresented as complete.

## Ordered plan

1. **Evidence inventory first:** compare Ticket 01, 02 and 03 requirements/acceptance lines against the 2026-10-12 manual/native WebDriver results, PR #1–#4 heads, GitHub CI and reviewer comments. Preserve their chronological provenance.
2. **Security + Standards/Spec two-axis review** of each exact stacked diff: main→#1, #1→#2, #2→#3. Prefer tests and source proofs over assumptions, and distinguish our review from an independent one.
3. **Minimal execution:** run non-invasive tests (source frontend and Rust core) in this clean review worktree, without rebuilding/replacing or launching the user's ordinary Tasknotes app, touching a personal SQLite database, invoking a WebDriver QA bundle, or exposing secrets. Fix only a reproducible new issue at a validated test seam; do not refactor speculatively.
4. **Reconcile documentation** with dated addenda, not rewriting the historic state. Keep Tickets 01–03 in `review` until independent review/merge authorization. Explicitly record outstanding manual OS dropdown path if applicable.
5. **Independent review:** inspect current GitHub review findings and request a legitimate available reviewer only if authorized and useful; do not call self-review independent, do not override previously exhausted review quota, and do not guess a reviewer identity.
6. **Deliver:** ensure CI pass on the new stacked documentation branch, commit and open Draft PR, recompute which tickets are actually unblocked. No merge, release or Ticket 04–18 implementation before parent acceptance status allows it.

## Boundary and risks

GitHub's main and stacked PR #1–#3 stay unchanged. In the original worktree, unfinished Issue19 QA edits and `MEMORY.md` are preserved. Tests must use synthetic data only. Status evidence cannot be strengthened by declaring the product release ready simply because ordinary Tasknotes feels smooth.