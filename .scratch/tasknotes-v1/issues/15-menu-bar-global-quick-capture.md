# 15: Menu bar and global Quick Capture

**What to build:** Optional macOS menu-bar mode, close-to-quit default, configurable global Quick Capture shortcut, shortcut conflict handling and user-visible settings. Verify app lifecycle behavior on Mac.

**Blocked by:** 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Closing the window quits by default when menu-bar persistence is not enabled.
- [ ] Optional menu-bar mode keeps Tasknotes available without a window and allows reopening the main UI.
- [ ] A configurable global shortcut opens Quick Capture from another app when permissions permit.
- [ ] Shortcut conflicts and permission denial show a comprehensible fallback without disabling ordinary Task creation.
- [ ] App quit/launch and persisted menu-bar settings behave consistently across restarts.

## Verification

- [ ] Verify native macOS tray/shortcut behavior through manual and supported automated acceptance checks.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-25 — owner decision (Han):** The global Quick Capture shortcut is **off by default**. The user turns it on and picks the key combination in Settings, so nothing conflicts with other apps until then.
