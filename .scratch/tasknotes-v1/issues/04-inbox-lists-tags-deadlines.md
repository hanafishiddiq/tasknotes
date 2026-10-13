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
- [ ] Complete **independent** Standards and Spec code-review and address proven findings before marking complete. Two independent reviewer attempts failed due to AGY credits exhausted (HTTP 429); implementing-assistant self-review is recorded separately in [Ticket04 review](../../../docs/reviews/ticket-04.md), but is NOT an independent approval.

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


## Ticket04 follow-up — viewport and reviewer gate (2026-10-13)

The initial source candidate 286482fb60e04dad8d0628372efc981215882cb2 created Draft PR #16. Subsequent bounded-viewport RED→GREEN changed the old accumulating "Load more" behavior into **at most 50 visible Task cards per page**, reversible Next/Previous, and scope-reset protection. A 120-Task synthetic Tauri contract test proves the rendered count stays 50/50/20/50/50. Additional RED→GREEN test confirms a rejected List rename retains its unsaved name draft.

Current positive tests: **30 Rust integrations**, **30 React**, TypeScript typecheck and frontend production build; strict Tauri and Rust Clippy/fmt previously passed with unchanged native source. See [acceptance report](../../../docs/qa/ticket-04-acceptance-2026-10-13.md) and [non-independent Standards/Spec self-review](../../../docs/reviews/ticket-04.md).

**Blocked gates:** AGY independent Standards and Spec reviewers both returned RESOURCE_EXHAUSTED (no actual review); the isolated native QA app rendered its GUI but WKWebView fields were inaccessible to AX, so actual click/type/reopen was not proven. Ticket stays **Implementation: review**; PR remains Draft and unmerged. Never migrate the owner's live SQLite without separately authorized recoverable backup and deployment.
