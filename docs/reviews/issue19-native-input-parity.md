# Issue19 — ordinary/QA separation, two-axis review

Date: 2026-10-13. Fixed point: `c8dc03f8f6f57eed75e9834798e574c7f27dcb84` (Draft PR #11). Branch: `test/issue19-native-input-parity`.

Reviewer: **implementing assistant, not independent approval**. The allowed `agy` CLI was checked and unavailable. No substitute identity or prohibited agent was used. Review uses `git diff c8dc03f...HEAD` once committed; before commit, the staged diff and new files are included explicitly.

Spec sources: `.scratch/tasknotes-v1/issues/19-native-interaction-lag.md`, the owner's new ordinary-smooth/QA-laggy report, and `docs/prototypes/issue19-native-input-parity-plan.md`. Standards sources: `AGENTS.md`, `GLOSSARY.md`, `docs/agents/issue-tracker.md`, existing QA safety/runbook conventions. No production behavior or domain rule is changed.

## Standards

**PASS at the changed build seam:** the hover runner supplies its dedicated absolute `CARGO_TARGET_DIR` directly to the build process, overriding an unrelated inherited target. Its bundle lookup uses the same directory. A missing isolated app exits with a precise rebuild instruction; no legacy shared-target fallback. Bundle-ID and temporary synthetic-data guards are retained.

**PASS, regression sensitivity:** two integration tests execute the real runner in disposable directories and stop through build-tool stubs before native launch or database access. They test observable target selection and fail-closed reuse, not a source-string assertion. Wrong-target and missing-bundle behaviors were observed RED then GREEN in an independent Linux sandbox. The tested runner/test hashes match the Mac files. This is not a claim that Mac unit execution passed; that invocation was tool-blocked.

**PASS, scope/privacy:** no changes to product React/CSS, Rust core/runtime/config or dependencies. Personal Task data and raw Inspector recordings were not accessed/copied into Git. Existing untracked `MEMORY.md` remains untouched. Diagnostic code is stored under `scripts/qa/experiments`, not loaded by Tasknotes or CI.

**LIMIT, experimental helper:** the Swift helper compiled and later completed one native Start trial after window visibility changed: 240 trusted synthetic wheel events, 3840px travel and focus retained. Earlier pilots and the later pacing control lost focus. It has a bounded timeout and labels synthetic input explicitly. Its startup-wide rAF data are not phase/presentation measurements. No speculative helper functionality is imported into the shipping application. A blocked visibility adjustment was not applied.

Standards verdict: no remaining hard violation identified in this scoped self-review; experimental/native-performance and independent-review limits remain explicit. Native isolated build/remote CI evidence is recorded separately when complete.

## Spec

**PASS, latest intent:** documents now distinguish the owner's ordinary-app smoothness acceptance from remaining diagnostic-app lag. Shared embedded JS/CSS hashes were verified in the actual binaries, not inferred from branch names. No cosmetic or scheduler workaround was applied to an app the owner now says works.

**PASS, demonstrated defect only:** shared raw debug output really contained the QA binary instead of the ordinary app. The runner fix prevents this specific collision. It is not presented as the cause or cure of physical scrolling lag.

**PARTIAL, native diagnosis:** direct in-process AppKit scroll dispatch reached the synthetic WebView's browser wheel/default scroll path. A native Start trial eventually completed with focus retained. However, the subsequent no-capture input-pacing control was invalidated by focus loss before completion, so the visual-sampler prerequisite remains unmet. No actual GPU-present FPS, physical trackpad attestation or valid clean-versus-instrumented visual comparison resulted. Tool restrictions were not bypassed.

**OPEN:** QA visual root cause, workload-wide product performance gates and independent review. The owner's accepted ordinary build must be preserved; no merge/release is authorized. The historical physical-capture runbook is not automatically reimposed as a prerequisite to using it.

Spec verdict: narrow build-isolation correction and evidence reconciliation meet their tested scope; original end-to-end diagnostic/physical-performance requirements remain incomplete at a real native-observation boundary. Do not describe this as all scrolling fixed.

## Completed build and regression verification

The dedicated native bundle built and the actual runner launched it; own-window startup showed Ready with 32 synthetic Tasks. The ordinary app hash is unchanged; the shared raw output was restored from that verified ordinary executable without launching it or accessing data. Both push/PR CI runs at `c0b2bd3` passed all 69 tests, typecheck/build/format/clippy. The final helper instrumentation compiled but its pacing trial was correctly inconclusive. Capture permission preflight succeeded; a visual sampler was not built atop the invalid pacing result. All exact values and limits are in the linked normal-versus-QA report. This is not independent review or a physical-lag fix.
