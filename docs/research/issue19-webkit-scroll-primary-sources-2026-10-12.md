# /research — Tasknotes Issue #19: native WKWebView scroll lag

**Date:** 2026-10-12
**Question:** Which evidence-supported debugging approach can distinguish *actual user-visible scrolling jank* from WKWebView callback throttling, and which design differences are worth prototyping before changing Tasknotes?
**Scope:** macOS 27.0.1 ARM64, Tauri 2.12.3, Wry 0.57.0, Tasknotes's main WKWebView.
**Status:** Research completed; **no production fix or physical frame-presentation measurement yet**.

> Research methodology: Official Apple, WebKit, Tauri, Rust Tauri source and Wry upstream first; use dated local Tasknotes/ChatGrid evidence for our system-specific observations. This is a direct primary-source investigation because the current Mac has no provisioned `agy`, `codex` or `claude` research subagent. It should not be described as a delegated background-agent report.

## 1. Definition and verified existing evidence

**User symptom:** Tasknotes native macOS scrolling feels extremely heavy, subjectively comparable to **10–20 FPS**. That is an observation, **not** a measured display FPS.

**Already measured on synthetic data:**

- Tasknotes's native React/Tauri GUI created/edited Tasks and persisted progress/Next Step across process restart in two independent end-to-end runs. No functional CRUD failure explains scrolling.
- An experimental native W3C-wheel/JavaScript `scrollBy` test returned `requestAnimationFrame` (rAF) callback p95 of ~117–121 ms for 1 or 30 Tasks **and for a blank scrolling document** after removing Tasknotes DOM/CSS. The red threshold `<50 ms` was a diagnostic threshold, **not** an approved product frame-rate target.
- A separate *standalone Swift/AppKit+WKWebView* executable (no Tauri, React, WebDriver or SQLite) reproduced focused rAF p95 ~120 ms while an `NSScreen.displayLink` delivered callback p95 ~16.8 ms on a 60 Hz screen.
- An A–B–B–A native debug/release comparison used identical frontend and synthetic DB: Debug scroll rAF p95 117/118 ms; optimized Release 106/113 ms, only ~6.8% mean improvement. Release compilation alone is not a demonstrated fix.
- An A–B–B–A main-vs-child view comparison using one same QA binary per profile found no meaningful benefit (Release top mean 218.5 ms, child 217.5 ms). Child with ChatGrid-like `background_throttling(Disabled)` did not help.
- **Important unresolved confounder:** Absolute rAF times differed greatly between separate QA harnesses (~110 vs ~220 ms), which differ in creation mode, compile feature, startup, and sometimes window focus. Those numbers are not a proven causal finding about declarative-vs-programmatic windows.
- Capture-based display-presentation experiments disturbed focus/timer scheduling and did not produce a trustworthy physical FPS estimate. Other user-owned WebContent processes and host swap were observed, but no causal effect was isolated.

**Local source pointers (exact repo evidence):**
`docs/qa/issue19-native-lag-diagnosis-2026-10-12.md`;
`docs/qa/issue19-matched-native-debug-release-2026-10-12.md`;
`docs/qa/issue19-main-vs-child-webview-2026-10-12.md`;
`docs/qa/issue19-chatgrid-astra-scroll-comparison-2026-10-12.md`;
`.scratch/tasknotes-v1/issues/19-native-interaction-lag.md`.

### Evidence must not be conflated

1. The refresh rate of the screen, `NSScreen.displayLink` callback rate, rAF cadence, Safari Web Inspector *rendering frame tasks*, Core Animation presentation cadence, and *perceived physical trackpad smoothness* are **different measurements**.
2. A standalone WKWebView with a slow rAF callback but healthy native display link narrows the issue to some aspect of WebKit scheduling **in the tested environment**. It does **not** prove that Tasknotes's physical display is rendering at ~8 FPS, or that a particular Apple/Wry bug is the root cause.
3. Focus is a first-class test validity condition. In earlier QA, an inactive/hidden WKWebView throttled JS timers to ~1 second. Never pool such invalid runs with foreground measurements.

## 2. Primary-source research findings

### Finding A — Tauri's two window-creation paths converge on the same builder

In the **exact installed Tauri 2.12.3 source**, default app startup iterates configured windows and invokes:

`WebviewWindowBuilder::from_config(app.handle(), window_config)?.build()?`

This happens **before** the user-provided `.setup()` hook.

`WebviewWindowBuilder::new(manager,label,url)` creates its own `WindowBuilder::new` and `WebviewBuilder::new`, whereas `from_config` composes `WindowBuilder::from_config` and `WebviewBuilder::from_config`. Both eventually use a `WebviewWindowBuilder`. Therefore declaring them different *rendering engines* is unsupported. Differences to measure are default field values, the precise `WindowConfig` values, startup/setup ordering, focus, and feature flags.

**Primary sources:**
- Installed `tauri-2.12.3/src/app.rs`, lines **2687–2698** (auto creation and setup ordering), from Cargo registry.
- Installed `tauri-2.12.3/src/webview/webview_window.rs`, lines **101–106, 150–155** (the two constructors).
- Public Rust Tauri 2.12.3 API: https://docs.rs/tauri/2.12.3/tauri/webview/struct.WebviewWindowBuilder.html
- Official Tauri configuration semantics: https://v2.tauri.app/reference/config/

**Implication:** The paused programmatic-vs-declarative QA experiment should not simply rerun previous binaries and announce a rendering-architecture win. **First** enumerate the effective config/default and lifecycle differences and design one controlled test to isolate exactly one of them.

### Finding B — Safari/WebKit offers a more direct rendering profiler than rAF

WebKit's **Web Inspector Timelines → Frames view** groups activity by browser rendering frame. The **Layout & Rendering** timeline records style recalculation, layout, compositing and painting; **JavaScript & Events** exposes JS/event work. The WebKit team explicitly describes separate frames/times and approximately 16.7-ms budget at 60 Hz.

The profiler itself can add overhead (the Media & Animations timeline documentation warns of profiling overhead), so matched baseline and inspected runs are needed. It is a **browser rendering-work profiler**, not automatically a physical compositor/GPU presentation recorder.

**Primary sources:**
- WebKit Timelines/Frames reference: https://webkit.org/web-inspector/timelines-tab/
- Original WebKit Rendering Frames explanation: https://webkit.org/blog/3996/introducing-the-rendering-frames-timeline/
- Apple `CADisplayLink` documentation: https://developer.apple.com/documentation/QuartzCore/CADisplayLink

**Implication:** Before trying implementation workarounds, collect a **focused, real Tasknotes WKWebView Rendering Frames trace** with synthetic Tasks. If long JavaScript, style/layout, or paint tasks dominate, identify their exact sources; if those are all short but rAF cadence is low, inspect window focus/scheduling/system compositor separately.

### Finding C — Native WKWebView inspection can be enabled per view, without full Xcode Instruments

Apple's `WKWebView.isInspectable` is `false` by default; when set `true`, its content becomes available from **Safari → Develop** on the Mac. Official Apple documentation shows this as an explicit per-view opt-in, and says it can be toggled during the WKWebView's lifetime.

Wry 0.57.0 documents devtools enabled by default for debug builds; *release* devtools on macOS may need a feature invoking private APIs, which must **not** be enabled indiscriminately in a shipping app. Restrict inspection to a dedicated debug/QA build and synthetic task data. No requirement to install full Xcode is established for the Web Inspector path itself.

**Primary sources:**
- Apple: https://developer.apple.com/documentation/safari-developer-tools/enabling-inspecting-content-in-your-apps
- Apple: https://developer.apple.com/documentation/webkit/wkwebview/isinspectable
- Wry 0.57.0: https://docs.rs/wry/0.57.0/wry/struct.WebViewBuilder.html
- WebKit: https://webkit.org/web-inspector/timelines-tab/

**Precondition for a prototype:** confirm Safari Develop menu and this specific synthetic WKWebView appear and that the Frames timeline can collect a short trace. Do not change the normal release app, Safari privacy/security settings globally, or request full Xcode until a strictly necessary blocker is demonstrated.

### Finding D — Background throttling is not a general foreground scrolling accelerator

Official Tauri `backgroundThrottling` config describes what happens when a view is hidden/not in a window, with values disabled, throttle, suspend. This is not a documented "boost active FPS" switch. Prior Tasknotes QA already tested `disabled` on the main window and a ChatGrid-like child WKWebView; neither showed a meaningful improvement in rAF callback timing.

**Primary source:** https://v2.tauri.app/reference/config/#backgroundthrottling

**Implication:** Reject production changes to the default background throttling setting without better controlled evidence. It can waste CPU/power with no proven improvement to user-visible scrolling.

### Finding E — A nearby upstream macOS compositor report exists but is *not confirmed as this bug*

Wry upstream **issue #1848** (opened 2026-09-16) documents macOS 26 WKWebView frames becoming **stale/partially painted** while the DOM and accessibility state keep updating. It is still reported open as of this research. The issue's reproduction involves scrolling **and switching animated tabs**. Tasknotes's user describes very low smoothness, not stale/mixed frames. Shared platform is suggestive; **symptoms and causal mechanisms differ**.

**Primary source:** https://github.com/tauri-apps/wry/issues/1848

**Implication:** Track it as a related upstream lead only. Don't apply `translateZ(0)` or force repaint and call it fixed without a matching regression. Don't use private macOS WebKit APIs as unreviewed workarounds.

### Finding F — ChatGrid's successful old patch addresses a defect absent in Tasknotes

Direct dated ChatGrid source archives reveal an old `window.requestAnimationFrame` override with `setTimeout(16/33ms)` race, plus blanket animation suppression. Post-fix ChatGrid removed these. Tasknotes does **not** override rAF, and both apps currently use Wry 0.57.0. Historical ChatGrid release report was near 60 FPS scrolling, but it is not a matched experiment on today's Tasknotes.

**Primary sources:** ChatGrid local `backups/20260929-011935/source-before-chromium.tar.gz` member `src-tauri/src/lib.rs`; its later archived/current `src-tauri/src/lib.rs`; ChatGrid `docs/PERFORMANCE.md` and `reports/2026-10-11-memory-efficiency.md`.
**Project summary:** `docs/qa/issue19-chatgrid-astra-scroll-comparison-2026-10-12.md`.

**Implication:** Do not copy the Astra-era timer workaround-removal patch into a codebase where that timer shim does not exist.

## 3. Ranked decision map — research before prototype

| Priority | Candidate | Evidence for trying | Falsifiable question | Exit criterion |
| --- | --- | --- | --- | --- |
| **1** | **Debug/QA native Safari Web Inspector trace** | Apple/WebKit primary tooling; previous rAF != displayed FPS | Does Tasknotes scrolling spend >16.7 ms/frame on JavaScript/layout/paint, or are frame callbacks sparse despite small tasks? | Capture focused synthetic scrolling trace; report JS/style/layout/paint and inspector-overhead control. |
| **2** | **Controlled config/lifecycle builder differential** | Tauri exact source shows same `WebviewWindowBuilder`; from_config maps different defaults | With matched config fields, window focus and feature flags, does creation *timing* alone affect frame callbacks? | Reproducible A-B-B-A; same assets and data, focus valid, predeclared improvement rule. |
| **3** | **macOS/WebKit upstream investigation** | Independent Swift WKWebView also has sparse rAF; Wry issue #1848 related but not identical | Does a minimal focused WKWebView show the same long browser rendering tasks/presentation stalls without Tasknotes? | Independent matched Web Inspector traces; verified upstream issue only after symptom match. |
| **4** | Tasknotes CSS virtualization/DOM optimization | Useful for 50k Tasks later; no lag relief when QA DOM removed | Does profile show Task list layout/paint dominating true frame durations? | Only if profiler identifies concrete costly selectors/components. |

**Not recommended now:** native window rewrite, child-WebView migration, production `backgroundThrottling: disabled`, JS rAF monkeypatch, forced animation, or a release-only switch. All have failed or lack demonstrated benefit on Issue #19.

**Prior QA effect-size rule:** a putative performance workaround was considered a candidate only if callback p95 improved by at least **30 ms and 30%**, measured in focus-valid paired runs; this exploratory cutoff is not the app's approved FPS/product SLA. Do not treat a faster single run as proof.

## 4. The one question for a possible /prototype

> **Can an isolated Tasknotes debug WKWebView (synthetic data, no WebDriver) expose a Safari Web Inspector Rendering Frames recording that differentiates expensive JS/layout/paint work from frame-callback scheduling?**

A narrowly scoped, throwaway *instrumentation* demo could answer this question without changing product UI. Because Matt Pocock's `/prototype` skill specifically targets a **state-model/logic demo or multiple UI design variants**, and **this is neither**, its own contract suggests staying in **`/diagnosing-bugs` with a minimal red-capable harness**. Do not invoke `/prototype` mechanically; reserve it for an actual UI-design or logic-design question uncovered after inspection.

**Proposed gate before code:** determine if the built-in debug Wry inspector is already available. If it is, use existing tooling; if not, research safe per-view inspection enablement and ask permission before any global OS or private API change. Profile Tasknotes using only a synthetic QA bundle. No new app renderer, no private Apple calls.

## 5. Decision and boundaries

**Evidence-supported next step:** Safari/WebKit Rendering Frames profiling of real focused Tasknotes QA vs independent minimal WKWebView; *then*, if necessary, resume the parked config/startup differential with matched settings. This is more informative than tuning CSS or adding a second WebView.

**Currently parked experimental code:** Local branch `test/issue19-declarative-vs-programmatic`; original uncommitted code secured in Git stash labelled `PAUSED Issue19 declarative-vs-programmatic QA pending primary-source research 2026-10-12`. The research branch is independent; **do not merge or apply stash automatically** until selecting a justified experiment.

**Safety:** No source modification to normal Tasknotes or ChatGrid, no access to personal Tasknotes/ChatGrid data, no disabling macOS security, no enabling devtools in release, no global device settings changes. Issue #19 remains OPEN/High. Green functional E2E does not mean scrolling is smooth.

---

**Research file:** `docs/research/issue19-webkit-scroll-primary-sources-2026-10-12.md`
**Primary provenance:** source links and exact installed crate names/lines above; local project QA measurements are historical empirical context, not claims made by Apple/Tauri.
