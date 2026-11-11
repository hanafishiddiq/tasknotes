# Tasknotes — Specification, batch 3

**Status:** in progress. Each feature is settled with the owner in its own grilling round, then added here; the owner asked to take the features one at a time (2026-11-11).
**Source:** the owner's "Tasknotes Features" List, Tasks added on 2026-11-11 after using the batch-2 build.
**Issue tracker:** `.scratch/tasknotes-batch-3/issues/`. Book: `progress/tasknotes-batch-3.md`.
**Delivery:** as in batch 2 (Q1, 2026-11-11): one Draft PR per ticket with an AI review gate; merging, the single install build, real data and cache removal each need the owner's confirmation. One install build after every ticket is on `main`; the batch-2 build caches stay until then.

## Problem Statement

Using the batch-2 build, the owner found that Move to Removed looks like any other button, the yellow mark on a Task opened from the new Task toast is too loud and does not last, a Task has no right-click menu, and the Pomodoro page only shows history while the controls and lengths live elsewhere.

## Features, in working order

1. **Red Move to Removed** — Ticket 01. Settled (Q3, 2026-11-11).
2. **New Task yellow mark** — soft yellow without a border when opened from the toast, and a mark that lasts on a newly added Task. Next round. Conflicts with the 2026-11-09 decision (Q3/Q14/Q25: a toast instead of a lasting yellow mark); the owner decides before any change.
3. **Task right-click menu** — a few actions (Move to Removed, change work status, …). Later round.
4. **Complete Pomodoro page** — controls and lengths on the page, not only history. Later round.

Out of scope: Assign Task to People (waits for multi-user work).

## User Stories

1. As the owner, I want Move to Removed at the end of an open Task to read red (text and icon), with a faint red fill under the pointer, so it stands apart from Save; it stays a reversible move with no dialog. Notes keep their toolbar trash icon as it is.

## Implementation Decisions

- Alert Red is the existing `--red` token (DESIGN.md), already readable in both appearances. The hover fill mixes 8% red into the canvas: 12% and 10% drop the red label below 4.5:1 in light appearance (measured by `src/a11y-contrast.test.tsx`).

## Testing Decisions

- Stylesheet rules and contrast are checked on App.css (`src/stylesheet-a11y.test.tsx`, `src/a11y-contrast.test.tsx`); the component wiring by an App test. The look is confirmed in the owner's click check after the install build.
