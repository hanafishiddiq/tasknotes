# Issue #19 — Matched build-mode A/B, Standards and Spec review

**Branch:** `test/issue19-release-debug-comparison`, stacked above the Issue #19 diagnostic PR.
**Method:** Two manual review passes, not independent third-party approval.

## Standards

- **Pass: isolation:** All new instrumentation is confined to `qa/native` and `scripts/qa`, and a capability file omitted from standard Tauri config. The generated test overlay has a **different bundle identifier and product name**.
- **Pass: no unsafe WebDriver:** Neither native benchmark build enables the `native-e2e` Rust feature. No localhost driver or Mac-wide mouse injection is used.
- **Pass: data safety:** The runner verifies the QA bundle identifier, blocks symlinked database paths and non-synthetic titles, uses an exact test-only SQLite namespace, and reads results with SQLite read-only mode. No personal SQLite or ChatGrid session is touched.
- **Pass: matched frontend:** Frontend and benchmark SHA-256 are recorded and reused verbatim in both build profiles. Debug and optimized release share identical test task fixtures and sequential order A-B-B-A.
- **Pass: auditability:** Collected measures are tagged with native-window visibility/focus and scroll distance; QA outcomes distinguish valid from inconclusive. One focus-lost release run timed out and was excluded transparently.
- **Observation:** The current test frontend script uses `setInterval(16ms)`; when the window loses focus, macOS throttles the timer and a run may wait for its bounded timeout. A fail-fast focus handler would improve harness efficiency before routine automation. Do not change the old benchmark assets and still compare their SHA with previously recorded results.
- **Observation:** The fixed QA data store persists under its unique bundle identifier until explicitly cleaned. Do not wipe it without first validating the namespace and synthetic-only titles.

## Spec/diagnostic hypothesis

- **Pass:** One synthetic React UI, same 31 Tasks, default main WKWebView in both modes; optimized native release and regular debug were compiled.
- **Pass:** Four valid focus-preserved scroll timings recorded, release 106/113 ms versus debug 117/118 ms in p95 rAF intervals.
- **Rejected hypothesis:** The average improvement of 8 ms (~6.8%) does not satisfy the declared 30 ms / 30% meaningful improvement condition.
- **Limit:** No claim about physical displayed frames, real trackpad gestures, statistical significance or user-perceived smoothness; rAF callback p95 is a narrower proxy.
- **Limit:** Other ChatGrid WebContent CPU and host swap were significant in the environment. They are confounders, not a proven explanation of Tasknotes lag.
- **Next best experiment:** Tauri *child* versus *main* WKWebView, holding the native build profile/page/data/focus policy constant.

**Verdict:** Appropriate as a diagnostic-only Draft PR with the negative A/B result clearly recorded. Issue #19 remains open; independent reviewer acceptance and product fixes are not provided by this benchmark.
