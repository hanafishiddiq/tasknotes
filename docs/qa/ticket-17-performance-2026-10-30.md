# Ticket 17: large-dataset performance, 2026-10-30

**Status: incomplete.** Through the core, CLI and MCP, search and paging meet their targets. Nothing about the app on screen is settled: the Mac's screen was locked and asleep during the runs, so its window got no real frames and macOS throttled it. Scrolling is **not measured**, and search through the app is measured only in that throttled state, where the one-letter searches are over target. [The on-screen run](#what-is-still-open) decides both.

## Setup

- **Hardware:** Apple M5 (Mac17,3), 16 GB memory, macOS (Darwin 27.0.0).
- **Dataset:** `tasknotes-bench seed`, 50,000 Tasks, 10,000 Notes, 40 Lists, 200 Note Folders, 1975 links. Written through the core's public operations, seed `8386110886392067173`, content fingerprint `0fa6884722568054`. Every word is generated, so it holds no personal data. Ids and timestamps come from the core, so they differ per seed run; content and order of creation do not.
- **Build:** release builds on `feat/ticket-17-performance`. The core table was measured after the last code change to the core and this tool.
- **Cold behaviour:** "fresh process" is a new process's first query, opening the store included. The OS file cache stays warm, because emptying it needs administrator rights, so this is not a cold disk.

## Search and lists through the core

200 warm runs and 20 fresh processes per scenario, pages of 50. Raw results: [core.json](ticket-17-results/core.json).

**Target: search p95 under 250 ms. Met through the core: the slowest search p95 is 89.63 ms.**

| Scenario | Records on page | p50 ms | p95 ms | max ms | Fresh process p95 ms |
| --- | --- | --- | --- | --- | --- |
| tasks: first page, priority order | 50 | 0.56 | 0.97 | 1.46 | 1.3 |
| tasks: first page, newest order | 50 | 0.51 | 0.57 | 0.66 | 1.25 |
| tasks: one List | 50 | 1.25 | 1.37 | 1.68 | 2.3 |
| tasks: Inbox | 50 | 7.76 | 8.02 | 9.72 | 8.88 |
| tasks: Removed | 50 | 3.86 | 4.03 | 4.37 | 5.1 |
| tasks: walking 100 pages, total | 50 | 207.76 | 214.17 | 227.51 | 218.38 |
| tasks: filter by status and priority | 50 | 1.73 | 1.83 | 1.98 | 2.76 |
| tasks: filter by tag | 50 | 6.97 | 7.44 | 11.43 | 8.74 |
| search tasks: one letter (typing starts) | 50 | 64.1 | 64.78 | 67.47 | 66.92 |
| search tasks: two letters | 50 | 25.6 | 26.39 | 27.53 | 27.96 |
| search tasks: common word (20%) | 50 | 8.18 | 8.92 | 15.36 | 9.18 |
| search tasks: common prefix | 50 | 7.95 | 8.46 | 9.22 | 10.15 |
| search tasks: medium word (5%) | 50 | 2.81 | 2.98 | 3.23 | 4.25 |
| search tasks: rare word (0.1%) | 39 | 0.62 | 0.7 | 0.87 | 1.48 |
| search tasks: Progress Entry word | 50 | 5.62 | 5.91 | 6.5 | 6.94 |
| search tasks: two common words | 50 | 6.19 | 6.46 | 6.81 | 7.54 |
| search tasks: no match | 0 | 0.5 | 0.57 | 0.65 | 1.23 |
| search tasks: word in one List | 50 | 1.48 | 1.58 | 1.72 | 2.42 |
| search tasks: word and status | 50 | 7.51 | 7.76 | 7.92 | 8.88 |
| search tasks: word and tag | 50 | 12.13 | 12.43 | 12.97 | 13.38 |
| search tasks: word, newest order | 50 | 9.25 | 9.58 | 9.86 | 10.82 |
| notes: first page | 50 | 0.45 | 0.53 | 0.64 | 1.13 |
| notes: one folder | 27 | 0.45 | 0.5 | 0.6 | 1.17 |
| search notes: one letter (typing starts) | 50 | 88.21 | 89.63 | 94.75 | 90.93 |
| search notes: common word (30%) | 50 | 3.96 | 4.17 | 4.8 | 5.17 |
| search notes: rare word (0.2%) | 16 | 0.69 | 0.77 | 0.89 | 1.49 |
| search notes: no match | 0 | 0.44 | 0.52 | 0.66 | 1.16 |
| audit: first page | 50 | 0.36 | 0.43 | 0.54 | 1.02 |

### Bottleneck found and fixed

Before the fix, searching Notes for one letter took p50 2,005 ms and p95 3,070 ms. A short prefix expands to thousands of words, and each Note on the page was looked up through the search index, which repeated that expansion 50 times. Snippets are now cut in one statement that reads the match once (`crates/tasknotes-core/src/notes.rs`), giving p95 89.63 ms. `a_page_of_snippets_repeats_no_search_for_a_short_prefix` failed before the fix (50 Notes 267 ms, one Note 11 ms) and passes now.

## CLI and MCP

Raw results: [cli-mcp.json](ticket-17-results/cli-mcp.json).

| Call | CLI p50 / p95 ms (new process each) | MCP p50 / p95 ms (one server) |
| --- | --- | --- |
| Task list, first page | 5.69 / 6.82 | 1.35 / 1.63 |
| Task list, common word | 14.78 / 15.42 | 9.14 / 9.5 |
| Task list, one letter | 70.91 / 72.2 | 67.52 / 69.53 |
| Note list, common word | 25.85 / 26.71 | 20.38 / 23.6 |

**Paging: no process loads the whole dataset.** Walking every record by cursor, the largest single answer is 100 records.

| Walk | Pages | Records | Largest answer | Peak memory |
| --- | --- | --- | --- | --- |
| CLI, Tasks | 495 | 49493 | 100 | 7.8 MB per process |
| CLI, Notes | 97 | 9674 | 100 | 5.6 MB per process |
| MCP, Tasks | 495 | 49493 | 100 | 16.7 MB, server |
| MCP, Notes | 97 | 9674 | 100 | 16.7 MB, server |

Record counts are below 50,000 and 10,000 because Removed items and, for an AI, Notes marked Exclude from AI are left out. A limit above the maximum is refused (`invalid_input`) by both. The first CLI command of a day also takes the Backup: 1241.7 ms here.

## The app

Raw results: [ui-screen-locked.json](ticket-17-results/ui-screen-locked.json). **These ran on a locked screen in a hidden window.** macOS throttles such a window, so its timings say little about using the app, in either direction. Nothing here counts as passing or failing a target.

- **First Task row after the page loads:** 173 ms here, 83–173 ms across runs. An unpainted window does less work, so this may be optimistic.
- **Memory:** 111.3 MB for the app process plus 224.2 MB for its WebKit helper processes, without painting. Helpers are found as WebKit processes that appeared after launch.
- **Rows in the page:** 16 of the 50 on the page were in the DOM at rest. Rows while scrolling are not measured.
- **Search through the app's IPC:** the slowest is one letter, Tasks p95 326 ms and Notes p95 405 ms. That is over 250 ms but throttled; the core takes 64.78 ms and 89.63 ms for the same queries.
- **Typing to settled results, 250 ms debounce included:** one letter in Tasks p50 918 ms, a whole word p50 322 ms.
- **Opening a row:** p50 15 ms, but p95 2193 ms. One slow open in 12 needs checking on screen before it is called throttling.
- An earlier run reported 60 FPS. It is discarded: the display was asleep for most of it.

## What is still open

1. **Scroll near 60 FPS with collapsed and open rows: not measured.** Run with the Mac unlocked and the screen on, leaving the window alone for about three minutes:

   ```bash
   scripts/qa/run-ticket17-ui.sh
   ```

   The scroll is scripted: each frame sets `scrollTop`, which is the costly main-thread path, so it is an upper bound on what a trackpad costs. It says nothing about how a finger on a trackpad feels; that stays in the owner's click-checklist.
2. **Search through IPC, typing to settled results, opening a row and memory while painting:** same run.
3. **A truly cold disk:** not measured, as above.
