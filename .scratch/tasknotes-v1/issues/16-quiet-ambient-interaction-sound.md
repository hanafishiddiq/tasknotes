# 16: Quiet ambient and interaction sound

**What to build:** Offline ambient and interaction sounds default ON at 8% and 12% respectively; separate sliders, mute, persistence, minimized-window behavior, quit behavior, graceful autoplay fallback. Include license and asset-quality review evidence before bundling audio.

**Blocked by:** 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** not started — requires an explicit phase-transition instruction.

## Acceptance criteria

- [ ] Interaction sounds and ambient sounds start enabled at 12% and 8% independent default volume respectively, subject to macOS playback permissions.
- [ ] A user can instantly mute either channel, change volume separately and preserve choices on restart.
- [ ] Chosen bundled audio loops pleasantly, works offline, and has a recorded license and redistribution review before packaging.
- [ ] Ambient playback behavior is correct on minimized window, close-to-quit and optional menu-bar states.
- [ ] Autoplay restrictions do not cause sudden loud fallback playback, excessive CPU utilization or broken UI controls.
- [ ] Reduce Motion and accessibility settings do not disable essential task functionality when audio is muted.

## Verification

- [ ] Run Mac playback/looping/license checks, app lifecycle tests and CPU sampling with audio on/off.
- [ ] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-25 — owner decision (Han):** The default ambient sound is **soft rain**, synthesized in code (Web Audio), so no third-party recording or license is needed. Record how it is generated as its provenance.
