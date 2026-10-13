# Issue 19 — ordinary Tasknotes is user-accepted; separate the QA failure

Date: 2026-10-13. Base `c8dc03f8f6f57eed75e9834798e574c7f27dcb84`. Branch `test/issue19-native-input-parity`.

## Current result, without conflating three questions

| Question | Evidence/status |
| --- | --- |
| Is ordinary Tasknotes still visibly lagging for the owner? | Owner explicitly reports ordinary **Tasknotes is now smooth**, while **Issue19 still lags**. This is qualitative native acceptance of the current ordinary build, not an agent-measured FPS result. |
| Was the difference caused by a different production React/CSS patch? | **No difference in the embedded application assets**: the same compressed blobs exist in both actual binaries and decompress to the same current dist files. This does not match their data, native settings or Inspector state. |
| Is the remaining QA lag's cause fixed/proven? | **No.** A new in-process native-input seam delivered browser wheel events and real scroll movement, but its performance trials lost foreground visibility. No valid matched visual-performance result exists. |

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

The final compiled but interaction-unverified helper is archived under `scripts/qa/experiments/issue19-inprocess-wheel-probe.swift`, not loaded by Tasknotes or CI. Do not treat its rAF samples (which include startup/idle) as phase-timed or physical FPS. No native performance PASS, physical trace, capture video or screen FPS is claimed from these pilots.

## 4. Research and decision

WebKit explicitly designs scrolling to remain independent of main-thread work (https://trac.webkit.org/wiki/Scrolling). Its Inspector documentation warns that Media & Animations, Memory and JavaScript Allocations timelines can affect performance and describes screenshot collection after composites (https://webkit.org/web-inspector/timelines-tab/). The public AppKit responder seam is documented at https://developer.apple.com/documentation/appkit/nsresponder/scrollwheel(with:). DOM `isTrusted` does not attest to hardware origin (https://dom.spec.whatwg.org/#dom-event-istrusted).

Therefore sparse rAF or an instrumented app's lag cannot invalidate the owner's ordinary-app acceptance on their own. No production animation switch, WebView rewrite, throttling workaround or global setting change is justified by this session's evidence.

## 5. Remaining boundary

To diagnose the separate QA visual lag further requires a reliably visible, focus-valid native test window and a matched visual-presentation comparison, or a permitted noninterfering native measurement facility. Repeating old scripted traces, lowering performance thresholds or relabelling synthetic input as physical will not supply that evidence. The current tools did not sustain that valid native comparison. The ordinary app remains the owner-accepted build; the remaining QA/root-cause and independent-review gates stay open.

## Final checkpoint

Pending native build, source-scope readback and remote CI; update only from their actual results. Keep Draft.
