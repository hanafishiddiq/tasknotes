# Tasknotes v1 — Local Issue Tracker

**Planning:** COMPLETE (2026-10-11). All Q1–Q49 decisions accepted.
**Implementation:** Tickets 01–03 have been implemented in stacked Draft PRs, remain under acceptance/review; Tickets 04–18 have not started. The user reported a new native-performance bug (#19).
**Canonical specification:** [spec.md](spec.md).
**Source of truth for terminology:** [GLOSSARY.md](../../GLOSSARY.md).
**Architecture decisions:** [docs/adr](../../docs/adr).
**Tickets:** [issues/](issues/), individual Markdown files with `Blocked by`, `Status`, `Implementation` and acceptance criteria.

## Work selection

1. Read `AGENTS.md`, the specification, relevant ADRs and the ticket's blocker list.
2. When the user authorizes implementation, select an **unblocked** ticket whose blockers have `Implementation: completed`.
3. Follow `/implement` with `/tdd` and `/code-review`. Record reproducible evidence and completion in that ticket's Markdown file.
4. Recompute the frontier from the ticket files; do not bypass blockers or count a status label as completion.
5. Never mix private runtime data into the public repository. Do not silently change decisions approved in Q1–Q49.

## Incoming defects

- **Bug 19 — Native UI lag**: [issue](issues/19-native-interaction-lag.md), high priority, `needs-triage`. Must be diagnosed and resolved before Ticket 17's performance gate and Ticket 18's release-quality sign-off. This incoming bug is **not** a silently added v1 feature requirement; the approved product plan still has 18 feature tickets.
- **Native GUI acceptance report**: [2026-10-12 evidence](../../docs/qa/native-acceptance-2026-10-12.md).

## Dependency graph

| Ticket | Outcome | Blocked by |
| --- | --- | --- |
| 01 | First Task on desktop | None |
| 02 | Status, priority and sorting | 01 |
| 03 | Progress history | 02 |
| 04 | Inbox, Lists, tags and deadlines | 01 |
| 05 | Search and filters | 02, 03, 04 |
| 06 | Markdown Notes | 01 |
| 07 | Note folders, pins, search | 06 |
| 08 | Task–Note links | 04, 06 |
| 09 | Removed and restore | 04, 06, 08 |
| 10 | CLI Task CRUD and concurrency | 03, 05, 09 |
| 11 | CLI Note CRUD and AI privacy | 07, 08, 10 |
| 12 | MCP stdio and bulk safeguards | 10, 11 |
| 13 | Backup, restore and export | 09, 11 |
| 14 | Desktop UX and accessibility | 05, 07, 09 |
| 15 | Menu bar and global Quick Capture | 14 |
| 16 | Ambient and interaction sounds | 14 |
| 17 | Performance and synthetic benchmarks | 05, 07, 12, 14 |
| 18 | macOS release-quality verification | 12, 13, 15, 16, 17 |

**Initially unblocked when implementation begins:** Ticket **01** only. After 01, independent Tickets 02, 04, and 06 can proceed.

## Privacy and public repository rules

- Every fixture is synthetic and contains no personal Tasks, Notes, backups or credentials.
- `Excluded from AI` Notes must not be emitted by AI-facing CLI/MCP operations or their error messages.
- Only explicit human-controlled export can include private Notes. No HTTP listener or cloud sync in v1.
- Removed items are reversible; do not add physical deletion without a new explicit decision.

## Benchmark targets

- 50,000 synthetic Tasks, 10,000 synthetic Notes.
- Search p95 below 250 ms on the target Mac; scroll performance close to 60 FPS.
- Performance remains a measured acceptance gate, **not** a guaranteed outcome before tests.
