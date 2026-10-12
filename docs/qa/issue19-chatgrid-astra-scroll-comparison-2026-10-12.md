# Issue #19 — ChatGrid smooth-scrolling source archaeology

**Date:** 2026-10-12
**Scope:** Evidence-only comparison. No changes to ChatGrid or Tasknotes runtime.
**Source:** Local ChatGrid project at `../chatgrid`, its September source archives and October reports. ChatGrid folder does **not** contain a `.git` directory, so historical attribution is grounded in dated archives and reports, not Git commits or authorship metadata.

## Why ChatGrid is relevant

The user remembers that ChatGrid's macOS browser became dramatically smoother after an intervention by Astra and asks whether Tasknotes can reuse that solution.

## Confirmed September 29 scroll fix

Primary sources:

- ChatGrid `docs/PERFORMANCE.md` and `README.md` (§ Performance update).
- Old source backup `backups/20260929-011935/source-before-chromium.tar.gz`, member `src-tauri/src/lib.rs`.
- Post-fix backup `backups/20260929-090811-layout/source.tar.gz`, same file.
- Current `src-tauri/src/lib.rs`, constant `CHATGPT_PERFORMANCE_SCRIPT`.

**Old source:** the initialization script overrode `window.requestAnimationFrame` and `window.cancelAnimationFrame`. It raced native rAF with a `setTimeout` fallback of **16 ms after user input/wheel events** and **33 ms otherwise**. It also intercepted `Document.prototype.hidden` and `visibilityState`, and globally forced essentially zero transition/animation durations. It affected injected ChatGPT pages, **not** a Tauri core rAF implementation.

**After fix:** the override/race and blanket animation overrides were removed; native WebKit animation timing was left intact. The current code preserves actual visibility state. Only targeted performance CSS remains: `content-visibility: auto` on conversation turns and disabling backdrop blur on particular ChatGPT sidebar elements. The current performance-injection script has **48 lines**, versus **120** in the old archive.

ChatGrid documented results on **2026-09-29, macOS 27.0, Apple M5, optimized release binary, 60-Hz screen**, from a WebKit JS rAF benchmark:

| Scenario | Documented mean FPS |
| --- | ---: |
| One authenticated session, idle | 59.98 |
| One session, scrolling | 59.65 |
| Three sessions, scrolling | 58.65 |

The report states that the older rAF/`setTimeout` race drove frames off the display vsync clock. These historical FPS figures are not a current normal-app profiling result or physical compositor frame-capture measurement.

## October 11 ChatGrid memory/CPU improvement

Primary source: ChatGrid `reports/2026-10-11-memory-efficiency.md`, especially § Historical pre-install baseline, § Implemented locally and § Round-1 activation and native results.

A **separate** improvement eliminated high-frequency deep DOM clones, HTML serialization and repeated unnecessary geometry queries in injected timer/recovery observation scripts. A synthetic WebKit benchmark went from **1,109 ms to 129.5 ms** for 120 accelerated observation ticks (different workload from scrolling).

Native before/after activation of the signed app showed the four WebContent processes' median combined memory decreasing **6.57 GiB → 3.46 GiB** (~47.4%). A restart, process age and changing pages confound causality. The user confirmed scrolling and typing felt “Normal dan lancar,” but the report explicitly says native FPS and matched-age before/after responsiveness **were not measured** in that run. This does not establish a 47% FPS improvement.

## Direct comparison to Tasknotes

| Dimension | ChatGrid smooth build | Tasknotes Issue #19 |
| --- | --- | --- |
| WebKit Rust wrapper | **Wry 0.57.0** | **Wry 0.57.0** |
| Tauri | 2.12.0 | 2.12.3 |
| WebView placement | External `chatgpt.com` **child** WKWebViews in a Tauri window | Local **main/top-level** Tauri WebView |
| Native child throttling | `WebviewBuilder.background_throttling(Disabled)` | Standard main WebView; a **QA-only** `backgroundThrottling: disabled` A/B did not improve scroll-callback cadence |
| rAF monkeypatch from before Astra | Removed in fixed source | **No rAF/cancelAnimationFrame replacement to remove** |
| Global animation suppression | Removed | No equivalent general override in normal UI. `prefers-reduced-motion` affects CSS only when requested |
| Massive authenticated DOM observation | Injected recovery/response timer, optimized Oct 11 | Absent; 1 synthetic Task still shows delayed rAF in QA |
| Release/debug | ChatGrid FPS benchmark on **optimized release** | Tasknotes scroll benchmarks so far predominantly **debug + opt-in WebDriver QA**; not a matched release comparison |
| Confirmed normal scrolling smoothness | User observed “normal dan lancar” Oct 11; historical release benchmark near 60 FPS Sep 29 | User reports severe scrolling lag (~10–20 FPS subjective estimate), test-only WKWebView rAF p95 ~117–121 ms |

**Important:** A completely separate Swift/AppKit+WKWebView synthetic test (no Tauri, React, WebDriver, database or Tasknotes source) also reproduced a focused rAF callback interval around **120 ms**, while native `NSScreen.displayLink` stayed near **16.8 ms**. See `docs/qa/issue19-native-lag-diagnosis-2026-10-12.md`. This makes the exact September ChatGrid shim fix **not applicable** to Tasknotes as written; copying it would add code without addressing a demonstrated shared defect.

## Ranked next experiments (diagnosis only)

1. **Normal release versus debug Tasknotes A/B**, with equally focused visible windows, the same synthetic scrollable data and comparable host load. ChatGrid's historical 60 FPS numbers came from an optimized release build. **Prediction:** if build mode is decisive, release yields materially faster visual frame delivery than debug. Keep testing on isolated data and do not overwrite any personal Tasknotes install.
2. **Main WKWebView versus child WKWebView under matched build settings**, in a minimal Tauri/WebKit project, because ChatGrid uses child views with explicit background-throttling configuration. **Prediction:** a child WKWebView configured identically to ChatGrid schedules frames more frequently than the normal top-level WebView. Do not introduce a child WebView architecture into Tasknotes until this differential has been measured.
3. **MacOS/WebKit version or compositor behavior** on a verified foreground normal app, comparing with the independent Swift control. **Prediction:** a minimal focused WKWebView shows the same timing pathology as Tasknotes, but a non-WebKit native display link does not. This is already supported at the callback level but *physical presentation FPS* remains unverified; `xctrace` is not available with Command Line Tools alone.
4. **DOM/CSS/SQLite workload cost**, lower priority because the test-only slowdown remains with Tasknotes DOM and CSS entirely removed. A large-DOM list optimization like ChatGrid's `content-visibility:auto` is appropriate for later 50,000-task performance work, but no evidence supports it as the immediate 1-task scrolling fix.

**Decision:** Do **not** copy the ChatGrid pre/post September source into Tasknotes. Do **not** mark Issue #19 fixed. Preserve the real red-capable scroll benchmark and the source-diff evidence; pursue a matched release/debug native profile, then a child/top-level WKWebView differential if release still shows slow scrolling.

## Source integrity / safety

- Historical ChatGrid snapshots were read directly from `.tar.gz`, never extracted into or written back to the live ChatGrid project.
- No authenticated ChatGrid WKWebView was reloaded, injected into, benchmark-scrolled, or changed.
- No personal Tasknotes SQLite was accessed. All findings are static source/reference analysis and pre-existing synthetic QA data.
- No Git metadata exists in the local ChatGrid directory; do not claim a specific commit authored by Astra without another primary record.
