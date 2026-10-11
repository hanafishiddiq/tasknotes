# 05: Indexed Task search and filters

**What to build:** Search title, description, and progress notes; filter by work status, priority, List, and tag; maintain fixed status groups under alternate sorts, exclude Removed, and paginate via indexed core queries. Validate results from the UI.

**Blocked by:** 02 — Work status, priority, and fixed ordering, 03 — Progress history and Next Step, 04 — Inbox, Lists, tags, deadlines

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Search finds Tasks by title, description, Next Step and Progress Entries, including terms in older history.
- [ ] Users can combine status, priority, List and tag filters and choose supported secondary sorts while status-group order stays fixed.
- [ ] Task query results are page/cursor bounded; successive pages do not repeat or skip items under a stable query snapshot.
- [ ] Removed Tasks do not appear in ordinary search; an explicitly requested Removed view can find them.
- [ ] Indexed query access and incremental UI rendering remain functional with a large synthetic fixture; measured release benchmark is performed again in Ticket 17.

## Verification

- [ ] Run deterministic indexed query tests and desktop search/filter/ordering E2E tests with synthetic data.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
