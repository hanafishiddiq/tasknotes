# Tasknotes — autonomous native WKWebView acceptance

> **Evidence ported as documentation only:** The native WDIO implementation remains on Draft PR #4, NOT in production main. This 2026-10-12 report describes two genuinely executed, synthetic native QA runs; porting the report neither installs nor runs the WebDriver plugin. [Source-only merge evidence](tickets-01-03-postmerge-2026-10-13.md).

**Date:** 2026-10-12
**Branch:** `test/native-wdio-e2e`, stacked on Ticket 03, `main` unchanged.
**Host:** macOS arm64, Tauri v2 WKWebView, temporary synthetic SQLite.
**Verdict:** PASS, two full repetitions. One diagnostic run initially FAILED on a hidden-window WebDriver select; the driver-only fallback was diagnosed and retested.

## Observed commands and results

```text
pnpm qa:native:build
  PASS — opt-in debug-only Tauri app with embedded WebDriver.

pnpm qa:native  # first successful full repetition
  Spec 01 — PASS (1.1s), real WKWebView → React → Tauri IPC → Rust → SQLite
  Spec 02 — PASS (270ms), fresh process reopened same synthetic database
  Read-only SQLite — PASS, one Task, four journal entries, exact Progress Note

pnpm qa:native  # independent fresh synthetic dataset, second repetition
  Spec 01 — PASS (3.6s), includes same state transitions and sort controls
  Spec 02 — PASS (259ms), fresh process and full history reload
  Read-only SQLite — PASS, same invariants
```

All test sessions created a fresh private TMPDIR directory named `tasknotes-wdio-*`, and successful runs deleted it automatically. No live personal database was opened.

## What the automated suite proves

- Create Task, edit title/description, save and reopen via native macOS WebView and real Rust/SQLite.
- Persist status/priority/progress and Next Step.
- Dated Progress Note `Native acceptance passed` really reaches SQLite (4 append-only entries).
- Completed 100% → In Progress 60% retains the last incomplete percentage.
- Newest/Priority sort inputs send changes through the app and are reflected in WebView state.
- Data persist through an actual terminated native app process and a second launched app process.

## Limitations and release implications

- Embedded WDIO's native `selectByAttribute` did not commit in hidden WKWebView; the scoped DOM input/change fallback was needed. It exercises React+IPC but not the OS mouse/dropdown picker.
- **Performance Bug #19 is unresolved**. Quick hidden automation does not measure human-perceived input/scroll delays under normal visible usage.
- No claim of independent Matt Pocock Standards/Spec review; GitHub Copilot quota was exhausted.
- Ticket 01–03 implementation stays `review`, with Draft stacked PRs unmerged pending independent review/merge decisions.
- Test plugin must never ship: debug-only compile feature, localhost WebDriver and synthetic-data path checks; normal production bundle must omit it.
