# 14: Desktop interaction and accessibility polish

**What to build:** All Tasks default, expandable one-at-a-time rows, empty states, keyboard navigation/Cmd+K, light/dark mode, Undo, focus visibility and Reduce Motion. Test that inline edits and scroll state are preserved.

**Blocked by:** 05 — Indexed Task search and filters, 07 — Note organization and searching, 09 — Removed and restore

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Tasknotes opens All Tasks with navigation to Inbox, Lists, Notes, Removed and Settings; empty states contain no fake user Tasks.
- [ ] Task details expand inline one-at-a-time without losing scroll position; inline edits and progress animation do not block keyboard input.
- [ ] Cmd+K, standard keyboard navigation, clear focus indicators and Undo cover the agreed major actions.
- [ ] Light/dark mode, readable contrast, semantic priority symbols and macOS Reduce Motion are respected.
- [ ] Task rows use virtualized rendering with dynamic height measurement; responsive interaction under sizeable fixtures is demonstrated.

## Verification

- [ ] Run macOS Tauri E2E navigation, keyboard, focus, screen-state and Reduce Motion checks.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments
