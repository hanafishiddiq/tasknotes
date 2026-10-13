# Issue19 — triage and scope decision

**Date:** 2026-10-13. **Base:** `6f3c8953e4f6df947c847b7b46b6dfc8e8a53047`; existing Draft PR #12. **Scope:** documentation only.

## Define the problem

The owner explicitly confirms **ordinary Tasknotes is smooth**, while the separate **Tasknotes Issue19 QA** application still lags. The current product symptom should no longer be described as occurring on the owner-accepted ordinary build. We have no demonstrated product code root cause or shipping fix; past experiments concern the diagnostic environment. Product performance across 50,000 Tasks and 10,000 Notes has not been approved.

## Evidence and decision gates

The historical real Safari capture, matched synthetic experiments, owner-visible native Tasknotes acceptance, and byte-identical application JS/CSS are documented under `docs/qa`. Prior launch/callback findings do not prove actual screen FPS or causal resolution. Under `docs/agents/triage-labels.md`, `wontfix` is the explicit decision to **stop further product-irrelevant QA investigation**, not a claim of code repair.

## Planned execution (no production source changes)

1. Preserve and do not stage/modify the original dirty Issue19 experiment worktree; use only this clean, separate documentation branch.
2. Update the canonical Bug #19 triage status and implementation disposition with owner's direct observation; preserve original historical data. Close **the current ordinary-app bug pursuit**, defer QA-only lag rather than declaring technical root cause fixed.
3. Align tracker overview and README to remove stale claims that *ordinary Tasknotes currently lags*, while explicitly retaining independent Ticket 17/18 performance and release checks.
4. Verify Markdown references and diff/scope, fetch remote head again, commit, push, open a stacked **Draft** PR based on #12. Do not merge or release; do not launch QA, access personal SQLite, or demand another physical Safari capture.

## Completion

This phase is done when the user-facing product status is accurate, the abandoned QA-specific investigation is unmistakably documented, and no performance/release ticket is mistakenly claimed complete.