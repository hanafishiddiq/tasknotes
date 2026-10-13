# Review plan — Tasknotes v1 Tickets 01–03 (Matt Pocock /code-review)

Date: 2026-10-13. Source: `AGENTS.md`, `GLOSSARY.md`, `.scratch/tasknotes-v1/spec.md`, local tracker and existing ADRs. Parent: stacked acceptance Draft PR #14 commit `4ee9cd4e50d0ab054651e44835448b6a6180fc78`. This review branch contains no new application behavior.

## 1. Define the remaining problem

Feature tickets 01–03 are already implemented and have direct native + two E2E synthetic-WKWebView acceptance runs and successful frontend/Rust tests. However, GitHub PRs #1–#3 still have only a quota-exhausted automated reviewer comment and no actual independent evaluation. Do not misrepresent self-review or green CI as an approval. Ordinary Tasknotes is owner-reported smooth, and Issue19 QA investigation is deliberately stopped. Feature source and current native bundle must not be altered solely to fix the QA program.

## 2. Breakdown (fixed points)

| Ticket | Fixed point | Head | Spec |
| --- | --- | --- | --- |
| 01 | `e4c728c2d175404a6de79ad2bc207eb40a9e0874` (`main`) | `b799ccec61ab4a43943b182d63936999cc6d1d97` | `.scratch/tasknotes-v1/issues/01-first-task-on-local-desktop.md` |
| 02 | Ticket 01 `b799ccec61ab4a43943b182d63936999cc6d1d97` | `bc8ac501e21677c1010c2a242b1518df53646ab8` | `.scratch/tasknotes-v1/issues/02-work-status-priority-fixed-order.md` |
| 03 | Ticket 02 `bc8ac501e21677c1010c2a242b1518df53646ab8` | `2937c9a0c8abb1beed5e799a3cc96c07aae18053` | `.scratch/tasknotes-v1/issues/03-progress-history-next-step.md` |

Diff command per ticket is `git diff <base>...<head>` (three dots); verify before dispatch. Review context: latest accepted native WDIO evidence in Draft PR #4 and reconciliation in Draft PR #14; do not quietly conflate their tests or code with the feature diffs. Standards: repository `AGENTS.md`, glossary, ADRs, review instructions, `/code-review` smell baseline.

## 3. Plan, before substantial execution

1. Recheck remote branch SHAs, tests and review permissions. Preserve original dirty Issue19 worktree, independent accepted ordinary app, and every Draft PR.
2. Check `agy` separate-agent CLI in **read-only/sandbox/plan** mode. Do not bypass its permissions, impersonate another GitHub owner, or label a failed model call independent. If usable, dispatch **two isolated agents concurrently** with identical immutable base/head/spec paths: Standards (repo coding rules and explicit Fowler smell heuristics) and Spec (required/missing/wrong/scope creep). Request exact file/line or diff hunk evidence and separate per-ticket findings.
3. Verify every returned finding against actual source and tests. For a concrete defect, first reproduce at the agreed public domain/UI seam (Rust `TaskStore`, React typed Tauri contract); only add a regression test red→green if necessary and do it on the correct parent-feature branch, not by modifying the running normal app. Do not start a speculative refactor.
4. Review both axes side by side **without blending their independent rankings**. Record the source SHA, agent model, boundaries, findings, true status (independent agent review vs independent human GitHub approval), CI results and exact outstanding gates. Existing 2026-10-12 native acceptance is reusable; no forced manual replay.
5. If review supports acceptance, prepare an unmerged Draft review PR/comment. **No merge/deploy without explicit user authorization**, and never mark Ticket 02/03 or begin Ticket 04/06 until parent dependency gates are fulfilled. Finish with a concise readiness decision or a concrete blocker.

## 3a. Least-privilege reviewer interface

Agy CLI is now installed and its read-only smoke test passed. Its headless sandbox denied the Git `command` permission for the first two audit calls. Those calls produced **no review findings**. Do **not** use `--dangerously-skip-permissions` or alter the user's global agent settings.

Instead, the host-side trusted Git client materializes exact fixed-point **tracked public-code diffs**, ticket/spec/standards text and native acceptance excerpt into six self-contained read-only prompt snapshots under the ignored `src-tauri/target/review-tickets-01-03/`. Each independent reviewer receives its **own single ticket/axis prompt**, with all necessary source text included verbatim; explicitly **no tool calls**. Require file+hunk and spec citations, then independently validate findings against source and actual tests. Any reviewer output without actual code comparisons is inconclusive, not an approval.

## 4. Controls and completion evidence

- No personal SQLite, credential, local user's note, screenshot, machine input replay, broad filesystem scan, remote deploy, release or non-target repo changes.
- Reviewer agents read **public source only**. No `--dangerously-skip-permissions`; reject unavailable independent tool instead of fabricating reviewer identity.
- Any test must use the already agreed public Rust/React seams and synthetic fixtures; a failing unrelated native QA app is not a shipping feature regression.
- Preserve `main`, PR #1–#4, #13/#14, and original uncommitted QA experiments. Use branch `review/tickets-01-03-independent-20261013`.
