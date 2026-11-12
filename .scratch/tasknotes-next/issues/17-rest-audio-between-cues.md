# 17: Rest the audio device between cues when no rain plays

**What to build:** With interaction sounds on and no rain playing, the sound engine suspends the audio device a few seconds after the last click, key press or cue, and wakes it for the next one, so an idle window costs no audio CPU (spec story 23, the fix proposed in Ticket 08 and chosen by the owner on 2026-11-12).

**Blocked by:** 08 — Measure what muted sound costs (done; its numbers are the reason)

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [ ] With no rain playing (muted or at 0 %) and interaction sounds on, the audio device is suspended a few seconds after the last click, key press or cue, and nothing wakes it until the next one; Settings shows no hint for it.
- [ ] A click or key press wakes the resting device, so the cue that follows plays at once; a cue that comes while it still rests plays as soon as the device is awake rather than being dropped.
- [ ] While rain plays, the device never rests.
- [ ] Kept as they are: a cue is still dropped while WebKit blocks autoplay or the system has interrupted audio; both channels muted still suspends at once; the quit fade and the waiting hint are unchanged.
- [ ] Native: with only rain muted and the window idle, the installed app's WebKit audio process uses about what it uses with both muted (Ticket 08 method), measured after the owner approves the install.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` and no install without the owner's confirmation.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-12 — owner's choice:** "Ya, perbaiki" to Ticket 08's proposal: suspend the audio device when nothing is audible and wake it for the next cue, accepting that the first cue after a quiet spell may start a few milliseconds late.
