# Issue 19 — ordinary Tasknotes is user-accepted; separate the QA failure

Date: 2026-10-13. Base `c8dc03f8f6f57eed75e9834798e574c7f27dcb84`. Branch `test/issue19-native-input-parity`.

## Current result, without conflating three questions

| Question | Evidence/status |
| --- | --- |
| Is ordinary Tasknotes still visibly lagging for the owner? | Owner explicitly reports ordinary **Tasknotes is now smooth**, while **Issue19 still lags**. This is qualitative native acceptance of the current ordinary build, not an agent-measured FPS result. |
| Was the difference caused by a different production React/CSS patch? | **No difference in the embedded application assets**: the same compressed blobs exist in both actual binaries and decompress to the same current dist files. This does not match their data, native settings or Inspector state. |
| Is the remaining QA lag's cause fixed/proven? | **No.** A later native-Start trial completed 240 events with focus retained, but the subsequent no-capture input-pacing control lost focus before completion. No valid matched visual-performance result exists. |

Do not require the owner to repeat the previous physical Safari export merely because the separate diagnostic app remains slow. Preserve the ordinary-app acceptance report and investigate the remaining QA-specific problem separately. Release-quality performance across the approved workload and independent review are still unverified; no merge/release is authorized.

## 1. Actual installed/running artifact provenance

The ordinary app was `src-tauri/target/debug/bundle/macos/Tasknotes.app`, built 2026-10-12 18:22 +07. The distinct diagnostic app was `Tasknotes Issue19 Hover A B QA.app`, built 2026-10-13 11:19 +07. Both were real running processes when inspected; no personal database contents were read.

| Artifact | SHA-256 |
| --- | --- |
| Ordinary native executable | `6055b42deec803fe3ae34a67c145a0bf58c4a21690fe0f8a105f2a865bbb54ae` |
| Legacy Hover QA executable | `f0b688980cdd355def39e65cd3153939f70c9066cea652be4852ee4e97c8fc17` |
| Embedded app JavaScript, 243233 bytes, **both binaries** | `78edd264cde7dca18000d0cad0d36048e85d41687089de73096186587524bcb9` |
| Embedded CSS, 14188 bytes, **both binaries** | `a282ac798bf2a78cfdfacb729288c3e4320b43644fb070c047c42d978397c839` |
| Additional QA module, 15335 bytes, **QA only** | `8e61fe93134c1f4657a97a492622625d3c44f7211b3a3fe5d9353e8552992155` |

Method: enumerate generated `tauri-codegen-assets`, find each complete compressed blob in the actual Mach-O files, decompress it with Node zlib and compare byte-for-byte with `dist/assets`. Matching asset filenames were also present in both binaries. This is stronger than guessing from Git HEAD or modification times, but is not a claim of identical runtime/fixture/Inspector conditions. The QA module, its loading HTML and global Tauri API are absent from the ordinary binary.

## 2. A demonstrated artifact collision, distinct from the scrolling root cause

Before correction, `src-tauri/target/debug/tasknotes` had the **QA executable's** exact hash, not the ordinary app's hash. The hover QA runner used the shared Cargo target directory. Thus an ordinary `.app` launch and a raw debug-binary launch could silently select different applications. This is a verified build-isolation defect; it is **not proof that the QA script caused the physical lag**.

The runner now fixes `CARGO_TARGET_DIR` to the repository's `src-tauri/target/issue19-hover-native` and launches its bundle from there. It overrides an inherited target for this build command only. `--reuse-bundle` refuses a missing dedicated bundle instead of falling back to an old shared-output app. Existing synthetic-data and bundle-ID checks are unchanged. No production CSS, React, Rust, app config or dependency change.

### TDD at the actual runner's external build boundary

`test_issue19_build_isolation.py` runs a disposable copy of the real shell runner with stubbed external build tools. The stubs stop execution before any app launch or database access.

- **RED 1:** observed inherited Cargo output instead of the dedicated QA path (`AssertionError`, expected path differs).
- **GREEN 1:** observed the dedicated absolute QA target even with an unrelated inherited target.
- **RED 2:** missing isolated bundle exited 1 with no useful operator message.
- **GREEN 2:** exits 2 with `Missing isolated QA bundle`; a legacy shared-output bundle is not reused and no QA data marker is created.

These two red/green tests were executed in an **independent Linux sandbox**, with runner and test source hashes verified identical to the Mac files. A Mac invocation of the unit suite was blocked by the tool safety gate and is **not** reported as run. No protected Mac action was recreated in that sandbox; only disposable fake build commands ran.

Final runner SHA256: `c312a81816921f3c92b2bb206f70034b78224e795c1476f2c5830bddc4f3f4c2`.
Test SHA256: `0a62637c60b40bd49c3cac1ef923ec7a332f337c36c3899d776a5fbeba039eaf`.

Native build and CI are recorded in the final checkpoint below when actually complete.

## 3. New native input seam, not another JavaScript scroll replay

The experimental Swift/AppKit program constructs a scroll event and calls the public `scrollWheel(with:)` responder on **its own synthetic WKWebView**. It never posts an event into the operating-system input stream, never controls Safari/other apps and never reads Tasknotes data. There is no JavaScript `scrollBy` or script-dispatched `WheelEvent` driving this test.

Observed short pilots:

| Pilot | Native events sent | Browser trusted wheel events | Travel | Valid performance comparison? |
| --- | ---: | ---: | ---: | --- |
| Initial own-view dispatch | 39 | 39 | 624 px | No: focus/visibility lost before the 240-event trial finished |
| Packaged native helper | 26 | 26 | 416 px | No: same validity failure |
| Durable execution, foreground diagnostic | 8 | 7 | 128 px | No: same validity failure |

These demonstrate that native events reach the browser's default scrolling path. They are **AppKit-synthetic**, not physical trackpad evidence. At the third rejection, the foreground application was `com.local.chatgrid`; the test app was inactive, its window was not key, and its window was not visible. This establishes the invalid test condition, **not why focus changed**, and not a ChatGrid product bug.

A native Start button was then added to avoid the inaccessible WebView controls. The helper compiled and displayed Ready, but scoped app activation timed out, and subsequent accessibility inspection did not expose its off-screen window/button. A proposed test-window visibility adjustment was blocked by the tool safety gate and **was not applied**; source readback confirmed no floating-window or all-spaces setting. That path stopped; no focus/input/security bypass was attempted.

The compiled experimental helper is archived under `scripts/qa/experiments/issue19-inprocess-wheel-probe.swift`, not loaded by Tasknotes or CI. Do not treat its rAF samples (which include startup/idle) as phase-timed or physical FPS. No native performance PASS, physical trace, capture video or screen FPS is claimed from these pilots.

### Later environment-qualified retry: native Start succeeded

After isolated QA startup, Mac observation newly reported both Tasknotes windows on-screen. Named activation produced a focused Ready screenshot. This changed condition justified one retry with the unchanged native helper, without the blocked visibility adjustment. Its native Start button was now accessible and the permitted semantic AXPress succeeded:

```text
nativeEventsSent=240; trusted=240; travel=3840px; scrollEvents=164
nativeFocusLost=false; documentFocus=true; visibility=visible
nativeInputSeamValid=true; physicalHardware=false; exit=0
```

This supersedes the earlier absence of a working native Start interaction. It proves complete synthetic native-input delivery, not hardware input or visual FPS. Startup-inclusive rAF data are not performance acceptance evidence.

Before constructing a visual sampler, read-only `CGPreflightScreenCaptureAccess()` returned true. Actual input elapsed time and already collected native-dispatch costs were then added to the helper, with a prospective input-pacing prerequisite of 3.5–6 seconds for 240 events at 60 Hz. The no-capture control failed before completion: focus switched away after 8/240 events, elapsed 0.569s, 128px travel, native dispatch p95 0.923ms. The document was hidden and ChatGrid was frontmost. This partial run proves neither normal pacing nor a slow-dispatch cause.

The planned visual sampler was consequently **not implemented or run**. Capture permission is available; sustained focus/visibility for matched input/presentation trials remains intermittent. No permission request, global input, forced-focus policy or floating/all-spaces change. The final helper with the extra metrics compiled successfully and remains experimental only.

## 4. Research and decision

WebKit explicitly designs scrolling to remain independent of main-thread work (https://trac.webkit.org/wiki/Scrolling). Its Inspector documentation warns that Media & Animations, Memory and JavaScript Allocations timelines can affect performance and describes screenshot collection after composites (https://webkit.org/web-inspector/timelines-tab/). The public AppKit responder seam is documented at https://developer.apple.com/documentation/appkit/nsresponder/scrollwheel(with:). DOM `isTrusted` does not attest to hardware origin (https://dom.spec.whatwg.org/#dom-event-istrusted).

Therefore sparse rAF or an instrumented app's lag cannot invalidate the owner's ordinary-app acceptance on their own. No production animation switch, WebView rewrite, throttling workaround or global setting change is justified by this session's evidence.

## 5. Remaining boundary

To diagnose the separate QA visual lag further requires a reliably visible, focus-valid native test window and a matched visual-presentation comparison, or a permitted noninterfering native measurement facility. Repeating old scripted traces, lowering performance thresholds or relabelling synthetic input as physical will not supply that evidence. The tools completed one focus-valid event-delivery trial, but could not sustain the next pacing prerequisite; a matched visual comparison is still absent. The ordinary app remains the owner-accepted build; the remaining QA/root-cause and independent-review gates stay open.

## Final checkpoint

- **Isolated native build PASS:** cold unsigned debug build completed in 12m34s. Bundle ID `store.adamaja.tasknotes.issue19hoverab`, under `src-tauri/target/issue19-hover-native/debug/bundle/macos/`. Native executable SHA256 `b866badfd7f2af4f1502a313949b79c676257be4b352a77d152204ae787df455`.
- **Actual runner/startup PASS:** the updated `--reuse-bundle` runner launched the isolated app; an own-window screenshot after named activation showed Ready and 32 synthetic Tasks. No hover/physical trial was started. This was a bounded smoke instance; no screenshot was committed.
- **Ordinary build preserved:** its hash remains `6055b42deec803fe3ae34a67c145a0bf58c4a21690fe0f8a105f2a865bbb54ae`. The shared raw executable was restored atomically from that exact ordinary bundle after checking its expected old QA hash and absence of a running raw process. Raw and ordinary now match. No ordinary rebuild/launch or personal-data access. The old agent-owned QA process was stopped; the ordinary app was left running.
- **Source-scope PASS:** no product React/CSS, Rust/core/runtime/config or dependency diff against `c8dc03f`.
- **CI PASS at implementation `c0b2bd3adc87eaed71df5b2ea9305f3b53120b3c`:** push `38349400228` and PR `38349450561`, frontend + core, four green jobs. Actual logs: **8 Node + 22 Python + 19 React + 20 Rust = 69 passing tests**, plus typecheck, build, Rust formatting and clippy.
- **Draft PR #12**, stacked on PR #11. The final diagnostic-metrics/evidence follow-up has its own CI readback. Review remains self-review; no merge or release. QA visual/root-cause and workload-wide physical-performance gates are still open.
