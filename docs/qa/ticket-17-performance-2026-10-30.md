# Ticket 17: large-dataset performance, 2026-10-30

**Status: incomplete.** Search, paging and memory are measured and meet their targets. Scrolling is **not measured**: the Mac's screen was locked and asleep during the runs, so a window gets no real frames. That part needs [the on-screen run](#what-is-still-open).

## Setup

- **Hardware:** Apple M5 (Mac17,3), 16 GB memory, macOS (Darwin 27.0.0).
- **Dataset:** `tasknotes-bench seed`, 50,000 Tasks, 10,000 Notes, 40 Lists, 200 Note Folders, 1975 links. Written through the core's public operations, seed `8386110886392067173`, content fingerprint `0fa6884722568054`. Every word is generated, so it holds no personal data. Ids and timestamps come from the core, so they differ per seed run; content and order of creation do not.
- **Build:** release builds of commit-tip code on `feat/ticket-17-performance`.
- **Cold behaviour:** "fresh process" is a new process's first query, opening the store included. The OS file cache stays warm, because emptying it needs administrator rights, so this is not a cold disk.

## Search and lists through the core

200 warm runs and 20 fresh processes per scenario, pages of 50. Raw results: [core.json](ticket-17-results/core.json).

**Target: search p95 under 250 ms. Met: the slowest search p95 is 96.42 ms.**

| Scenario | Records on page | p50 ms | p95 ms | max ms | Fresh process p95 ms |
| --- | --- | --- | --- | --- | --- |
| tasks: first page, priority order | 50 | 0.55 | 0.77 | 1.33 | 1.3 |
| tasks: first page, newest order | 50 | 0.53 | 0.67 | 4.7 | 1.26 |
| tasks: one List | 50 | 1.27 | 1.45 | 1.68 | 2.25 |
| tasks: Inbox | 50 | 7.85 | 8.18 | 8.81 | 9.21 |
| tasks: Removed | 50 | 3.94 | 4.17 | 4.87 | 5.09 |
| tasks: walking 100 pages, total | 50 | 221.54 | 249.3 | 579.06 | 232.61 |
| tasks: filter by status and priority | 50 | 1.83 | 1.93 | 2.0 | 2.9 |
| tasks: filter by tag | 50 | 7.91 | 8.71 | 11.64 | 9.02 |
| search tasks: one letter (typing starts) | 50 | 66.22 | 72.15 | 78.37 | 73.13 |
| search tasks: two letters | 50 | 26.79 | 27.76 | 29.59 | 28.54 |
| search tasks: common word (20%) | 50 | 8.43 | 8.68 | 9.13 | 9.84 |
| search tasks: common prefix | 50 | 8.39 | 8.62 | 9.12 | 9.8 |
| search tasks: medium word (5%) | 50 | 2.98 | 3.1 | 3.23 | 4.26 |
| search tasks: rare word (0.1%) | 39 | 0.65 | 0.73 | 0.86 | 1.59 |
| search tasks: Progress Entry word | 50 | 5.92 | 6.08 | 6.28 | 7.27 |
| search tasks: two common words | 50 | 6.49 | 6.7 | 6.98 | 7.98 |
| search tasks: no match | 0 | 0.52 | 0.58 | 0.67 | 1.36 |
| search tasks: word in one List | 50 | 1.57 | 1.65 | 1.8 | 2.56 |
| search tasks: word and status | 50 | 7.9 | 8.11 | 10.88 | 9.44 |
| search tasks: word and tag | 50 | 12.95 | 13.19 | 13.47 | 15.72 |
| search tasks: word, newest order | 50 | 9.86 | 10.06 | 10.2 | 11.33 |
| notes: first page | 50 | 0.51 | 0.6 | 0.67 | 1.28 |
| notes: one folder | 27 | 0.51 | 0.6 | 0.65 | 1.38 |
| search notes: one letter (typing starts) | 50 | 88.08 | 96.42 | 102.04 | 89.63 |
| search notes: common word (30%) | 50 | 3.94 | 4.09 | 4.35 | 5.27 |
| search notes: rare word (0.2%) | 16 | 0.69 | 0.76 | 0.84 | 1.54 |
| search notes: no match | 0 | 0.45 | 0.5 | 0.61 | 1.18 |
| audit: first page | 50 | 0.38 | 0.44 | 0.49 | 1.07 |

### Bottleneck found and fixed

Before the fix, searching Notes for one letter took p50 2,005 ms and p95 3,070 ms. A short prefix expands to thousands of words, and each Note on the page was looked up through the search index, which repeated that expansion 50 times. Snippets are now cut in one statement that reads the match once (`crates/tasknotes-core/src/notes.rs`), giving p95 96 ms. `a_page_of_snippets_repeats_no_search_for_a_short_prefix` failed before the fix (50 Notes 267 ms, one Note 11 ms) and passes now.

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

Raw results: [ui-screen-locked.json](ticket-17-results/ui-screen-locked.json). **These ran on a locked screen in a hidden window**, which macOS throttles, so they are an upper bound and not the feel of the app.

- First Task row after the page loads: 86 ms, 112 MB peak app memory. The page shows 50 Tasks at a time and keeps under 30 rows in the page.
- Through the app's IPC in the hidden window, the slowest search is one letter: Tasks p95 366 ms, Notes p95 444 ms. That is over 250 ms but throttled; the core takes 72 ms and 96 ms for the same queries. The on-screen run decides this one.
- An earlier run reported 60 FPS. It is discarded: the display was asleep for most of it.

## What is still open

1. **Scroll near 60 FPS with collapsed and open rows: not measured.** Run with the Mac unlocked and the screen on, leaving the window alone for about three minutes:

   ```bash
   scripts/qa/run-ticket17-ui.sh
   ```

   The scroll is scripted: each frame sets `scrollTop`, which is the costly main-thread path, so it is an upper bound on what a trackpad costs. It says nothing about how a finger on a trackpad feels; that stays in the owner's click-checklist.
2. **Search through IPC on screen, and typing to settled results:** same run.
3. **A truly cold disk:** not measured, as above.
