# Issue #19 — Safari Web Inspector native QA gate

**Date:** 2026-10-12
**Status:** **Inspector-ready QA built; trace subsequently exported by the user and analyzed.** This document preserves the historical pre-capture gate.
**Branch:** `research/issue19-webkit-scroll-sources` (research/diagnostics only).

## Exact target prepared

- Built a **normal, unsigned debug Tauri application** with the unchanged Tasknotes frontend and production Rust core, at ignored path `src-tauri/target/debug/bundle/macos/Tasknotes Issue19 Inspector QA.app`.
- Bundle identifier: `store.adamaja.tasknotes.issue19inspector`, **not** the normal `store.adamaja.tasknotes` identifier.
- The native app was launched successfully, with a separate `TASKNOTES_DEV_DATA_DIR` pointing to an isolated private `tasknotes-inspector-*` temporary folder. It contains an integrity-checked SQLite schema v3 **with exactly 31 synthetic Task titles starting `__ISSUE19_QA_AB__`**. No personal Task/Note database was opened.
- The app visibly rendered the complete Tasknotes UI and all 31 test Tasks. Mac window title: **Tasknotes Issue19 Inspector QA**.
- The app uses only the normal `default` Tauri capability. No embedded WebDriver, external listener, private inspector invocation, or release-build debugging configuration was added.
- Wry 0.57.0's **debug** default sets `WebviewAttributes.devtools=true`; Apple's public `WKWebView.isInspectable` property is thereby enabled (Cargo registry `wry-0.57.0/src/wkwebview/mod.rs`, lines ~537–546) calls public `WKWebView.setInspectable(true)` when available. Thus the *source* confirms the debug WebView is intended to be discoverable in Safari. **A Safari Develop-menu entry was not directly observed.** Wry's debug implementation additionally enables developer extras internally; we deliberately did **not** invoke Tauri `open_devtools()` (which calls a macOS private `_inspector` API).
- macOS QA window and frontend screenshot were inspected using the authorized app-scoped Mac observation tool. The source branch remains clean and the original normal app identifier was independently verified.

## Official Web Inspector recording workflow

**One-time owner-only UI boundary:** The available Mac controller can observe the Tasknotes QA native window, but cannot control Safari's Develop menu (browser automation is scoped to a different browser); global keyboard shortcuts are explicitly refused. Do not bypass this boundary using shell UI injection or private WebKit APIs.

1. Open **Safari**. If **Develop** is absent, go to **Safari → Settings → Advanced → Show features for web developers**. This is an Apple-provided UI option; do not change it via `defaults write` or other global settings scripts.
2. Go to **Develop → [This Mac] → Tasknotes Issue19 Inspector QA** (choose the inspectable `index.html` page). As an alternative, open **Develop → Inspect Apps and Devices**, then choose the same synthetic app. The target name might be shown under the current Mac's machine name.
3. In the Web Inspector, open **Timelines**, and select the **Frames** view (or Layout & Rendering / JavaScript & Events timelines). Start recording. Scroll the **synthetic** Tasknotes QA task list for about **8–10 seconds**, then stop recording.
4. **Export** the recording using Web Inspector's built-in Export command (or `⌘S` while the recording is active), using the file extension offered by Safari. Save only synthetic QA diagnostics, not tabs from personal sites.
5. Provide the exported trace file or its path. Analyze **JavaScript tasks**, **style/layout duration**, **paint/compositing work**, and dropped/long rendering frames. Use a nonrecording control for inspector overhead, and confirm focus was maintained.

The user has already completed CRUD/progress/restart acceptance. **Do not request any of those tests again.** This human step is strictly for opening and capturing the native Safari performance trace once.

### Expected scientific result / interpretation

- **If long JS/style/layout/paint tasks coincide with scrolling**, isolate the exact Tasknotes source selectors/functions and implement a targeted RED→GREEN regression.
- **If work within rendered frames is short but frame callbacks remain sparse**, narrow attention to WKWebView scheduling, WindowServer, display timing, foreground state or upstream Wry/WebKit behavior. Do not force CSS animations as a workaround.
- **If the target does not appear in Safari**, the Inspector-capability path is blocked even though source suggests inspectability: verify Safari developer features, QA app still running, and WebView version; do not conclude Inspector recording succeeded.
- **Rendering Frames timeline is not automatically physical screen FPS.** Existing rAF p95 (~100–220 ms under various QA conditions) is a different metric and cannot be equated to compositor presentation FPS.

## Reproducibility and cleanup

The **QA-only build config and data marker** are deliberately ignored local artifacts:

- `src-tauri/target/issue19-inspector/inspector.conf.json`
- `src-tauri/target/.issue19-inspector-data-dir`
- `src-tauri/target/debug/bundle/macos/Tasknotes Issue19 Inspector QA.app`

The app has **no modifications to tracked Tasknotes application source**. When inspection is over, terminate only the `Tasknotes Issue19 Inspector QA` process, then remove **only** its verified `tasknotes-inspector-*` temporary data directory if desired. Never delete or modify the user's normal `~/Library/Application Support/store.adamaja.tasknotes` directory.

The QA build overwrites the raw `target/debug/tasknotes` executable as a standard Cargo/Tauri build side effect, but the normal `Tasknotes.app` bundle and normal app-data directory remain separate. When using the raw debug executable again, rebuild the ordinary bundle with `pnpm tauri build --debug --bundles app --no-sign --ci`.

## Source citations (primary)

- Apple developer tools enabling instructions: https://developer.apple.com/documentation/safari-developer-tools/enabling-developer-features
- Safari Develop menu and Inspect Apps and Devices: https://developer.apple.com/documentation/safari-developer-tools/develop-menu
- WebKit Inspecting app WebViews: https://webkit.org/web-inspector/enabling-web-inspector/
- Official WebKit timeline / Frames recording and export: https://webkit.org/web-inspector/timelines-tab/
- Installed Wry 0.57.0 source, Cargo registry `src/wkwebview/mod.rs`, ~537–549, and `src/lib.rs`, default devtools in debug ~867–875.

**Gate outcome:** Inspector-ready synthetic target **PASS**; actual **Web Inspector trace PENDING** (unavailable through approved browser/macOS UI controls). **Issue #19 remains OPEN / High, no performance fix made.**

## Follow-up: Inspector trace successfully received and analyzed (2026-10-12)

The previous **trace PENDING** gate has since been satisfied by the user's latest Safari JSON export from Downloads. The recording was valid (14.073 s, 396 wheel events). [Quantitative Web Inspector analysis](issue19-safari-inspector-trace-analysis-2026-10-12.md) identifies expensive compositing events and a testable association with Task-card CSS hover transitions. **No physical FPS or specific root-cause fix is claimed.** The full 99 MB export stays local and outside Git.
