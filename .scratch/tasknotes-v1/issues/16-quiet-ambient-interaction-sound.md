# 16: Quiet ambient and interaction sound

**What to build:** Offline ambient and interaction sounds default ON at 8% and 12% respectively; separate sliders, mute, persistence, minimized-window behavior, quit behavior, graceful autoplay fallback. Include license and asset-quality review evidence before bundling audio.

**Blocked by:** 14 — Desktop interaction and accessibility polish

**Status:** ready-for-agent

**Implementation:** implemented and code-reviewed; on `integration/tasknotes-v1-07-18` (Draft PR #20), awaiting the owner's native check and merge decision. Plan: `docs/plans/ticket-16-ambient-sound-2026-10-26.md`.

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
- [x] Run `/code-review` and address findings before marking complete. *(findings fixed, see Comments 2026-10-26 — review fixes; the lifecycle criterion above stays open for Ticket 15 and the native check)*

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

Integration point for Ticket 15: superseded by the review fixes below (no presence API; only the quit fade matters).

For owner review: rain character and 12 s loop; cue sounds (create tick, complete chime) and which actions get one; 2 s fade-in; linear volume; "Ambient rain" label and mute-icon layout; rain continuing while minimized/menu bar.

Open: native macOS playback, looping, lifecycle and CPU checks (owner checklist in the plan).

**2026-10-26 — review fixes (implementer, Ticket 16).** Status unchanged.

Code-review findings and what each became:

1. First-click stall: the 60–80 ms rain render no longer runs on the first click. `rain.ts` renders in 16,384-sample slices with a pause between them (same samples as before, test-proven equal); the engine starts it as soon as ambient is wanted and the context exists, before any gesture, and starts the rain the moment it finishes. Muted ambient never renders it. Tests: slices equal the one-go render and pause repeatedly; render happens once before the gesture and the click only starts it; a slow render starts the rain on completion; no render while muted.
2. Interaction mute/volume now ramps over 80 ms (the channel starts at 0), so muting mid-cue does not click. Test: no hard `set` to 0, final ramp to 0 within 0.1 s.
3. Simplification: `Presence` and `setPresence` are gone (nothing called them but tests); one internal `quitting` flag remains. The hook no longer listens to page visibility. `addEventListener?.` became `onended`; the `Float32Array` cast became a typed render result; the fade duration is one `fadeSeconds()`; `sync()` is split into `sync` / `wake` / `applyLevels`.
4. Status flicker: after the engine's own suspend, un-muting resumes without showing "Sound starts after your first click…"; the hint shows only if the resume leaves audio blocked. Test records every status change across un-mute.
5. Ticket 15 integration: Ticket 15 is NOT in `integration/tasknotes-v1-07-18` yet (tip still 57d0ec6, nothing to merge). The lifecycle criterion therefore stays unticked; minimized/menu-bar need no code (rain simply continues) and quit goes through `saveBeforeQuit`, but this must be re-verified against Ticket 15's code once it is merged.
6. The plan now says why sound settings live in localStorage (the page alone reads them) while shell settings live in Rust (read with no page loaded).

Evidence (implementer's own runs): `pnpm test` 20 files / 214 tests passed (208 before; 6 new); `pnpm typecheck` and `pnpm build` clean. `src-tauri` untouched, cargo not run. Headless CPU and native checks not repeated.

**2026-10-26 — Ticket 15 integration note (implementer, Ticket 15).** Ticket 15 now merges this ticket's code. Proven by tests: hiding the window to the menu bar sends the page nothing and rain keeps playing (engine test, plus `src/sound.test.tsx` "keeps the rain playing while Quick Capture brings the window back from the menu bar"); a quit request fades the rain (existing App test). Closing with menu-bar mode off and the tray's Quit reach that quit request through `quit::request` in Rust, which is build-checked but not unit-tested. The lifecycle criterion stays unticked until the native check (Ticket 15 checklist item 10).
