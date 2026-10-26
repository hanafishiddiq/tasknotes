# 16: Quiet ambient and interaction sound

**What to build:** Offline ambient and interaction sounds default ON at 8% and 12% respectively; separate sliders, mute, persistence, minimized-window behavior, quit behavior, graceful autoplay fallback. Include license and asset-quality review evidence before bundling audio.

**Blocked by:** 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** implemented, awaiting independent review (branch `feat/ticket-16-ambient-sound`; plan `docs/plans/ticket-16-ambient-sound-2026-10-26.md`).

## Acceptance criteria

- [x] Interaction sounds and ambient sounds start enabled at 12% and 8% independent default volume respectively, subject to macOS playback permissions. *(`sound.test.tsx` defaults; `sound-engine.test.tsx` 8% rain and 12% cue; native playback still owner-checked)*
- [x] A user can instantly mute either channel, change volume separately and preserve choices on restart. *(80 ms mute ramp; persisted in `tasknotes.sound`; remount test)*
- [ ] Chosen bundled audio loops pleasantly, works offline, and has a recorded license and redistribution review before packaging. *(No audio file is bundled: rain and cues are synthesized in code, provenance in the plan, so no license review is needed and it is offline by construction. Seamless loop is unit-tested; "pleasantly" awaits the owner's listening check.)*
- [ ] Ambient playback behavior is correct on minimized window, close-to-quit and optional menu-bar states. *(Minimized/menu-bar keep rain, quit fades it — engine and App tests. Menu-bar and close-to-quit come with Ticket 15; integration point in the plan. Native check open.)*
- [x] Autoplay restrictions do not cause sudden loud fallback playback, excessive CPU utilization or broken UI controls. *(No ramp while blocked, fade from 0 on first gesture; headless Chromium ≈1% of one core playing, 0.1% muted; controls work without Web Audio)*
- [x] Reduce Motion and accessibility settings do not disable essential task functionality when audio is muted. *(`sound.test.tsx`: no Web Audio + Reduce Motion, mute and complete a Task)*

## Verification

- [ ] Run Mac playback/looping/license checks, app lifecycle tests and CPU sampling with audio on/off.
- [x] Relevant tests pass and evidence is recorded here. *(see Comments, 2026-10-26)*
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-25 — owner decision (Han):** The default ambient sound is **soft rain**, synthesized in code (Web Audio), so no third-party recording or license is needed. Record how it is generated as its provenance.

**2026-10-26 — implementer (Ticket 16), awaiting independent review.** Status unchanged.

Evidence (implementer's own runs on the branch, integration tip 57d0ec6): `pnpm test` 20 files / 208 tests passed (186 before; 22 new in `src/lib/sound/rain.test.tsx`, `src/lib/sound/sound-engine.test.tsx`, `src/sound.test.tsx`); `pnpm typecheck` and `pnpm build` clean. `src-tauri` untouched, so cargo was not re-run. CPU (headless Chromium 1248, real engine bundled, 20 s windows): idle 0.05%, rain + cues 0.9–1.0%, both muted 0.1% of one core; one-time rain render 62 ms (Chromium) / 82 ms (WebKit). WebKit CPU could not be attributed (audio runs in XPC processes).

Decisions (details in the plan): saya pilih synthesis for rain and cues karena nothing then needs a license review and it is offline by construction; a pre-rendered 12 s loop played from one buffer karena it keeps the audio thread to buffer playback; suspending the AudioContext when both channels are silent karena muted should cost nothing; dropping (not queuing) cues while autoplay is blocked karena a late burst is worse than silence.

Integration point for Ticket 15: `useSound().setPresence("menu-bar")`; policy keeps rain for minimized/menu-bar and silences it only on quit (the quit fade uses `saveBeforeQuit`). See the plan.

For owner review: rain character and 12 s loop; cue sounds (create tick, complete chime) and which actions get one; 2 s fade-in; linear volume; "Ambient rain" label and mute-icon layout; rain continuing while minimized/menu bar.

Open: native macOS playback, looping, lifecycle and CPU checks (owner checklist in the plan); `/code-review` not run yet.
