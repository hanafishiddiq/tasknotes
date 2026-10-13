# Issue 19 — exact physical-trial identity review and verification

Date: 2026-10-13. Fixed point: `d9121c0da27ce4081cc46ea3d03933991908b424` (parent Draft PR #10). Branch: `fix/issue19-physical-trace-identity`. Review command after commit: `git diff d9121c0...HEAD`; precommit review used `git diff d9121c0` plus the new test/plan files.

**Reviewer: implementing assistant, not independent approval.** The allowed `agy` CLI was not found on PATH or its standard application path. GitHub's live PR #10 reviews list was empty. No substitute account or prohibited agent command was used. Independent review remains pending.

## Standards

- The patch is restricted to the opt-in QA marker protocol, numeric comparator, synthetic tests, CI and documentation. The ID is generated once outside measured phases, added to the existing eight markers and persisted through the existing QA metrics path. It is a non-secret correlation value, not hardware attestation.
- Source `src/`, Rust core, native runtime/config/dependencies are unchanged versus the fixed point. No private Task data, raw Safari recording, screenshot, SQLite database or credential is included. The unrelated untracked `MEMORY.md` is preserved.
- The comparator keeps legacy all-unidentified recordings available for numeric-only analysis, but rejects missing/different IDs in strict physical mode and mixed phase identities. Duplicate and incomplete markers still fail closed. Existing focus, timing, trusted-event, travel, CSS and baseline criteria are unchanged.
- Review cleanup: random-ID generation is inside the existing error/finally boundary so a failure re-enables controls. Explicitly close QA SQLite readers/test connections; Python 3.14's observed unclosed-connection warnings are gone. No other refactoring or speculative architecture.
- CI now runs the previously local-only Node and Python QA tests, including the actual JS encoder → Python CLI → isolated SQLite integration test.

**Standards finding:** no remaining violation identified in this scoped self-review; not an independent merge approval.

## Spec

- The separate reproduced QA defect is addressed: a different trial with otherwise identical statistics cannot be mistaken for the latest physical trial. A full synthetic CLI test accepts only the matching ID, rejects a different or absent ID, and preserves legacy numeric-only analysis without candidate promotion.
- This does **not** fix, reproduce or measure the user's physical scrolling symptom. Synthetic protocol fixtures must never be presented as human/native performance evidence. A fresh Safari physical capture is required; old recordings cannot supply the missing observation.
- No CSS workaround, renderer rewrite, global setting change, input injection, release or merge. The original Issue #19 remains OPEN/High. Both-A expensive-composite reproduction, both predeclared 30% improvement criteria, uninstrumented native user acceptance and independent review remain required.

**Spec finding:** the narrowly planned evidence-integrity correction passes its automated seam; original scrolling acceptance remains BLOCKED on physical capture and measured improvement.

## Actually executed red → green evidence

1. Before correction, the different-ID regression failed with `AssertionError: ValueError not raised` (15 Python comparator tests, one failure). After the minimal identity check, 15 passed.
2. Before parser support, suffixed phase markers failed as missing all eight markers (16 tests, one error). After the parser change, all 16 passed.
3. Before the JS encoder existed, its test failed with missing export `phaseMarkerName`. After implementation, all 8 Node tests passed.
4. Full verification: **8 Node QA tests + 20 Python tests + 19 React tests + 20 Rust-core integration tests = 67 passing tests**. Python includes a full CLI cross-language protocol test with disposable synthetic SQLite. `pnpm typecheck`, `pnpm build`, `git diff --check` and the dedicated unsigned debug QA app build passed.
5. Scope check: `git diff --exit-code d9121c0 -- src crates src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/src` returned zero with no diff.

## Native artifact provenance

- Bundle: `src-tauri/target/debug/bundle/macos/Tasknotes Issue19 Hover A B QA.app`
- Verified identifier: `store.adamaja.tasknotes.issue19hoverab`
- QA JS SHA-256: `8e61fe93134c1f4657a97a492622625d3c44f7211b3a3fe5d9353e8552992155` (source, copied frontend and build manifest match)
- Original source CSS SHA-256: `49bbb907f0d235b9bc78f7a7b10f21e6cc48c9eda8eb3ac60a02845aedaca700`
- Native QA executable SHA-256: `f0b688980cdd355def39e65cd3153939f70c9066cea652be4852ee4e97c8fc17`
- No opt-in WebDriver feature was included in the build command. Actual serialization of the new markers by Safari during physical input is **not yet verified**.

The pre-change live QA report still contained three scripted runs (two focus-invalid, one valid), not a physical trial. The two existing Downloads recordings were not rerun as new performance experiments.

## Next exact gate

Use the rebuilt QA app and the [physical capture runbook](../qa/issue19-trusted-wheel-qa-runbook.md). Start a fresh Safari recording (not resume), disable Screenshots, physically perform the four labelled trackpad phases, export one new JSON to Downloads, then run the comparator with `--require-physical-qa`. Keep the PR Draft and the bug open. A candidate result is not final product acceptance.

## Native startup smoke check

Started the verified bundle with `bash scripts/qa/run-issue19-hover-ab.sh --reuse-bundle`. The own-window screenshot showed the real Tasknotes UI, 32 synthetic Tasks, Ready status and the separate **Start REAL trackpad A–B–B–A** control. No scripted or physical trial was started by the agent. This verifies startup/readiness only, not focus-valid scrolling or new Safari marker serialization. The screenshot was not added to Git.
