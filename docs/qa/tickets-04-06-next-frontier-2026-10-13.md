# Tasknotes v1 — next unblocked feature frontier after Tickets 01–03

**Date:** 2026-10-13. **Decision state:** research/roadmap only; no Ticket 04/06 implementation authorization or code changes from this phase.

After source-only merges PR #1 → #2 → #3, the local issue tracker has `Implementation: completed` for these three feature tickets. Dependency checks yield **two independent unblocked tickets**:

| Next ticket | Prerequisite | Feature objective | What it unlocks |
| --- | --- | --- | --- |
| **04 — Inbox, Lists, tags, deadlines** | 01 | List/Inbox ownership, Task tags, optional deadlines/overdue with time-zone-safe clock tests, bounded listing | **05 — Indexed Task search and filters**, and later Task–Note links |
| **06 — standalone Markdown Notes** | 01 | Rich-text editor round-trips to **canonical Markdown**, revision-protected autosave and restart-safe Note CRUD | **07 — Note organization and search**, plus future Task–Note linking and CLI/MCP Note functionality |

The dependencies **02 and 03 do not block 04 or 06** directly, but their already-integrated status/progress schema v3 must remain preserved by later migrations.

## Recommendation and proposed sequence

**Default next:** Ticket **04** as one complete vertical `/implement` slice. It extends the stable Task model without choosing a complex rich-text editor first, and removes the last blocker for Task search Ticket 05. In parallel, Ticket 06 can begin with a **research/spec seam check** of safe editor↔canonical-Markdown round-trip and autosave revision behavior; do not commit an editor choice before verifying supported formatting/serialization against ADR-0003.

### Proposed Ticket 04 TDD seams (to confirm when authorized)

- **Rust TaskStore + synthetic SQLite**: List CRUD / Inbox default, foreign-key ownership and task moves, tag association, deadlines with injectable clock/timezone. Migration from schema v3 must preserve all Task/Progress Entries without touching live user data.
- **Typed React/Tauri command seam**: users can create/move Tasks, edit tags/deadlines, see correct overdue state and unchanged In Progress→Ready→Completed grouping.
- **Native acceptance**: synthetic actual app create→move→restart; bounded pagination, null/deleted List reference rejection, deliberate clock-boundary cases. Historical QA Issue19 harness is not needed.

### Proposed Ticket 06 seams (to confirm when authorized)

- **Canonical Markdown serialization**: WYSIWYG edit/save/reopen preserves headings, bold, italic, lists, checklists, and links in the declared supported subset (ADR-0003).
- **Revision-safe Note core CRUD**: interrupted writes recover last consistent Note, concurrent autosave never silently overwrites a newer revision; Notes remain standalone and optionally attachable only in later tickets.
- **Native Note editor**: actual editing/autosave after restart using synthetic markdown fixtures; privacy and Exclude from AI remain future interface invariants.

## Safety and authorization

Both ticket source files say **not started** and explicitly **require a new phase-transition instruction**. This document records a grounded execution route, **not** permission to build both. Owner confirmation of the next ticket and its TDD seams is required by the repository's accepted workflow. No Tasknotes.app installation, data migrations, public notes, backup secrets or production feature merges occur here.

Release readiness is still gated by Ticket 17 synthetic 50k/10k performance and Ticket 18 macOS recovery/privacy/release verification. Issue19's QA-only lag pursuit remains stopped unless the owner sees lag again in the ordinary app.
