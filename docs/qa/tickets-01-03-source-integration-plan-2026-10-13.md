> **Historical pre-execution plan:** written before the feature merges, for reproducibility. Source merges #1–#3 are now completed; see [actual integration evidence](tickets-01-03-postmerge-2026-10-13.md). This plan is not another authorization or deployment task.

# Tasknotes v1 — source integration plan, Tickets 01–03

Date: 2026-10-13. Workflow: `/ask-matt` → fixed-point `/code-review` evidence → source-only serial integration → `/retro`. **Plan prepared BEFORE merges.**

## Definition

The owner replied **“lanjutkan, tetap berdasarkan workflow /ask-matt”** immediately after a direct question asking permission to accept independent AI reviews and merge feature PR #1 → #2 → #3 into `main`, explicitly **without deployment or personal SQLite migration**. Interpret this narrow contextual assent as permission for these three source merges only. Any other PR merge, app deployment, migration, actual user's data, release, auto-update, feature Ticket 04/06 implementation or broad environment change requires a separate decision.

Completed evidence: 2026-10-12 real native Tasknotes manual acceptance and two WKWebView E2E runs; 2026-10-13 six independent AI Standards/Spec audits, one separate adjudication, and 39 React/Rust source tests. Independent AI review does not equal a human GitHub APPROVED review; there is no documented branch rule requiring that status. The owner's immediate answer is taken as acceptance of this limited review policy for these three PRs. Keep it transparent; don't represent it as a signed human review.

## Preflight facts at planning time

- Remote `main`: `e4c728c2d175404a6de79ad2bc207eb40a9e0874`; PR #1 head `b799ccec61ab4a43943b182d63936999cc6d1d97`; PR #2 head `bc8ac501e21677c1010c2a242b1518df53646ab8`; PR #3 head `2937c9a0c8abb1beed5e799a3cc96c07aae18053`. Do not trust these after any remote change; re-read before each mutation.
- Actual ancestry validated: `main` → #1 → #2 → #3 → QA #4. Each PR is currently Draft, `MERGEABLE`, frontend/core push/PR CI four SUCCESS, GitHub reviewDecision empty; previous Copilot comments were quota exhaustion. GitHub `rules/branches/main` returned an empty list. Re-check branch protection and GitHub merge permission; never use `--admin`.
- Only `.github/workflows/checks.yml` (frontend/core CI) is present; source merge should not deploy by itself. No external provider deployment is authorized.
- Original development worktree contains unfinished Issue19 diagnostic changes and `MEMORY.md`; it must remain untouched. This separate, dedicated worktree is based on `origin/main`. The owner-reported smooth normal Tasknotes process must not be stopped or rebuilt.

## Exact ordered execution and gates

1. **Simulate the final integration** in this disposable local worktree: after the plan is written, merge the three pinned feature heads locally as test-only commits in order, verify their source tree exactly equals the reviewed Ticket 03 head. Confirm no Issue19-specific diagnostic code, QA WebDriver plugin, personal data, or unexpected GitHub workflow. Run frontend and Rust core tests, typecheck, build, fmt and clippy **only in this isolated worktree**, using synthetic/temp paths and Cargo in PATH.
2. **PR #1**: re-read remote main/head/CI, review results and diff scope; if exact expected values and all gates pass, mark #1 ready for review (Draft transition), then merge with GitHub merge commit preserving individual commits. Supply `--match-head-commit` and explicit repo; no `--delete-branch`, no `--admin`, no auto-merge. Verify PR merged, remote main advanced, and source tree equals PR #1 head.
3. **PR #2**: revalidate #2 pinned head, original base and CI; retarget base to *new main* after #1 merge, confirm GitHub diff contains **only** Ticket 02 and checks pass. Then mark ready and merge normally with `--match-head-commit`. Verify remote main/source tree equals PR #2 head and #3 remains untouched.
4. **PR #3**: same process for Ticket 03, after #2. Confirm forward-only v3 migration source present, but DO NOT execute on live user database. Verify remote main/source tree exactly equals PR #3 head; all required CI green and no unexpected files.
5. **Follow-up tracker**: once merged and verified, produce only documentation on a new branch from the current main updating completion statuses *accurately*: feature code merged, independent AI review logged, human OS picker and future release/large-dataset gates still distinct. Do not cherry-pick the abandoned Issue19 diagnostic chain. Decide separately whether to carry only the disposition text from PR #13 and the acceptance/review notes from PR #14; avoid pulling their QA parents.
6. **Frontier**: Ticket 04 and Ticket 06 become potentially unblocked once Ticket 01 completed; prepare next-step analysis from their approved requirements, but do not silently implement an extra ticket merely from merge authorization.

## Stop criteria and safety

If remote head, unexpected checks, reviews, branch rules, source tree, schema expectations or deployment behavior differs, STOP before the relevant merge, explain the obstacle, and preserve completed commits. Do not force push, bypass protection, rewrite/delete original feature branches, merge unrelated PR #4–#14, trigger UI tests or deploy an app. A source merge is not a rollbackable personal DB migration; backup/restore required before any future release touching real data. Record the actual commit SHAs and evidence, not a planned success.
