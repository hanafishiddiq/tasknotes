# Issue #19 — Main vs child WKWebView two-axis review

**Fixed point:** `c92438c` (prior release-vs-debug diagnostic result).
**Diff:** `git diff c92438c...HEAD` after committing the new QA branch.
**Method:** Manual Standards and Spec review. This is **not** independent subagent/human approval.

## Standards

- **Pass — production safety:** The new Rust module is behind `issue19-placement-qa` and Rust's `tauri/unstable` feature. Regular Tasknotes windows and data paths are unchanged. QA checks a distinct, hard-coded bundle identifier **before opening SQLite**.
- **Pass — environment separation:** The browser assets, generated overlay, logs and synthetic metrics reside under ignored `src-tauri/target/issue19-placement` or a dedicated `~/Library/Application Support/store.adamaja.tasknotes.issue19placement` namespace. Runner reads only synthetic titles through SQLite `mode=ro` and rejects symbolic-link database paths.
- **Pass — one-variable native comparison:** One Tauri/Wry binary can choose between `WebviewWindowBuilder` main and `Window.add_child(WebviewBuilder)` at runtime, controlling compile flags, viewport size, frontend resources and host process identity. The optional disabled throttling is tested separately from child placement.
- **Pass — test seam:** The real Tasknotes frontend, IPC and Rust/SQLite back end are exercised. Page logs visibility, first-responder focus, scroll distance and rAF p95; A–B–B–A randomizes neither, but partially controls time drift. Invalid focus pilot records excluded explicitly.
- **Pass — no network or external-window manipulation:** No embedded WDIO listener, cloud sync, private Task DB, OS-wide mouse injection or user ChatGrid intervention; test windows are synthetic and terminate after a bounded run.
- **Follow-up concern:** Programmatic window creation and opt-in `tauri/unstable` may account for baseline discrepancy versus earlier declarative-window QA, but neither mechanism was independently isolated. Preserve both baselines; never attribute the ~2× difference without another controlled experiment.
- **Cleanup gate:** Restore ordinary unsigned debug `Tasknotes.app` after QA; do not release the placement feature-enabled app. Maintain the normal CLI feature behavior and locked tests.

## Spec/acceptance

- **Pass — direct primary hypothesis test:** Debug top p95 220/215 ms versus default child 215/224; Release top 217/220 versus child 216/219. Same webpage, database, scroll length and native binary per profile; all included records focus-valid.
- **Pass — ChatGrid policy test:** Child WebviewBuilder with `BackgroundThrottlingPolicy::Disabled` yielded Debug 220/224 and Release 222/219. No measurable improvement.
- **Pass — predeclared improvement rule:** Candidate must reduce rAF p95 by 30 ms and 30%; child changes were about 2 ms slower in Debug and 1 ms faster in Release. Hypothesis **rejected under the tested conditions**.
- **Limit — physical behavior:** Synthetic timer-driven `scrollBy` and JS `requestAnimationFrame` are a browser scheduling proxy, not measured GPU frame presentation or physical Mac trackpad behavior. No actual user-facing smoothness fix has been demonstrated.
- **Limit — reviewer independence:** Automated suites, native QA evidence and this manual review do **not** close the independent Standards/Spec review gate for PRs #1–#6.
- **Result:** Keep Issue #19 OPEN/High, no architectural rewrite. Next experiment should isolate declarative vs programmatically created main WKWebView in an otherwise identical QA runtime.

**Review verdict:** This branch can be a stacked **Draft diagnostic PR** with a negative finding; no production merge and no bug-resolution claim.
