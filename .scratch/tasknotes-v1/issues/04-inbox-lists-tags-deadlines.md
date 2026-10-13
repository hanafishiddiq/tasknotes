# 04: Inbox, Lists, tags, deadlines

**What to build:** Organize Tasks in Lists/Inbox, add tags and optional deadlines, show overdue indicators, and persist CRUD changes across restart. Test List ownership and relative date display.

**Blocked by:** 01 — First Task on a local desktop

**Status:** ready-for-agent

**Implementation:** review — source and synthetic TDD tests implemented on a dedicated Ticket04 feature branch; native QA build, independent Standards/Spec review and merge/deployment approval remain separate gates.

## Acceptance criteria

- [x] Tasks can be created in Inbox or moved to a named List and still appear in All Tasks without duplication. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Users can add/remove tags and optional deadlines and see overdue indicators without violating the fixed status order. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] List/Task metadata and tag associations survive restart; nonexistent List references cannot orphan a Task. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Overdue behavior is deterministic with a controlled local clock/time zone in tests; no forced reminder is introduced. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [x] Large Lists are browsed through a bounded query/viewport, not an unbounded render. **Evidence:** [Ticket04 test report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).

## Verification

- [ ] Actual native GUI CRUD/quit/restart on Inbox, Lists, tags and deadlines still requires visible interaction acceptance. Rust referential-integrity, React/Tauri contract, synthetic native startup and clock-boundary tests have passed.
- [x] Frontend and Rust test suites pass, including v1/v2/v3 to v4 migration and locale-boundary controls; evidence [here](../../../docs/qa/ticket-04-acceptance-2026-10-13.md).
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments


## Implementation checkpoint — 2026-10-13

The user authorized Ticket04 implementation through the ongoing `/ask-matt` workflow after Ticket01–03 source merges. This is a TDD implementation branch **not** a deployed macOS app. The new local SQLite **schema v4** is forward-only, so no upgrade against real user Task data is permitted without a recoverable backup and separate owner approval.

**One-variable TDD history:** List schema/mutations, then Inbox/scoped Task views, then tag+deadline metadata, then React/Tauri command adapter and local-calendar overdue labels. The independent Issue19 QA lag investigation was **not** reopened. Removing a List is reversible and hides its Task children without deleting or mutating their work status/progress/history; the unified Removed view remains Ticket09's scope.

**Acceptance limitations:** The React mocked Tauri boundary and temporary SQLite Rust core behaviors are covered. A successful unsigned test-only Tauri bundle proves native compilation, **not** owner-operated GUI create/move/restart acceptance. A real native interaction with synthetic data and independent code-review findings remain explicit gates. No source merge/release authorized.
