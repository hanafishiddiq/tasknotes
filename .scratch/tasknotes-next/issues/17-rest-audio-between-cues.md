# 17: Rest the audio device between cues when no rain plays

**What to build:** With interaction sounds on and no rain playing, the sound engine suspends the audio device a few seconds after the last click, key press or cue, and wakes it for the next one, so an idle window costs no audio CPU (spec story 23, the fix proposed in Ticket 08 and chosen by the owner on 2026-11-12).

**Blocked by:** 08 — Measure what muted sound costs (measured 2026-11-12; its numbers reach `main` with PR #47)

**Status:** ready-for-agent

**Implementation:** in progress — Draft PR

## Acceptance criteria

- [x] With no rain playing (muted or at 0 %) and interaction sounds on, the audio device is suspended a few seconds after the last click, key press or cue, and nothing wakes it until the next one; Settings shows no hint for it. _Evidence: `src/lib/sound/sound-engine.test.tsx` "suspends the audio device a few seconds after the last cue once the rain is muted, and leaves it so" (status stays `playing`, which has no hint in `SoundSettings`; no resume for a further minute), "counts the quiet seconds from the latest click, key press or cue"._
- [x] A click or key press wakes the resting device, so the cue that follows plays at once; a cue that comes while it still rests plays as soon as the device is awake rather than being dropped. _Evidence: "wakes on a click or key press, so the cue that follows plays at once, with no waiting hint", "plays a cue that comes while the device rests as soon as it is awake, rather than dropping it"; a wake that takes longer than 250 ms drops the cue instead of playing it with the next click: "drops a cue whose wake is held back, rather than playing it late with the next click" (the fake now holds `resume()` the way WebKit's pending resume does)._
- [x] While rain plays, the device never rests. _Evidence: "never rests while the rain plays"; rain wanted but still rendering keeps it awake too (the rest needs `ambientTarget() === 0`; seen in a review probe; removing the rain checks from the rest sends the suite into a suspend-and-wake loop)._
- [x] Kept as they are: a cue is still dropped while WebKit blocks autoplay or the system has interrupted audio; both channels muted still suspends at once; the quit fade and the waiting hint are unchanged. _Evidence: the existing tests pass unchanged ("drops a cue rather than delaying it while autoplay is blocked", "suspends audio entirely when both channels are muted…", "fades the rain out before quitting…", "resumes after the system interrupts audio…", "does not flash the waiting hint…"); new: "treats an interruption during a rest like any other: the waiting hint shows and cues are dropped", "forgets its rest when disposed, so a new device that WebKit blocks still says it is waiting"._
- [ ] Native: with only rain muted and the window idle, the installed app's WebKit audio process uses about what it uses with both muted (Ticket 08 method), measured after the owner approves the install; and the first cue after a rest is heard (the owner's ear).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` and no install without the owner's confirmation.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-12 — owner's choice:** "Ya, perbaiki" to Ticket 08's proposal: suspend the audio device when nothing is audible and wake it for the next cue, accepting that the first cue after a quiet spell may start a few milliseconds late.

**2026-11-12 — built (`/tdd`), choices made in the code:**
- The rest comes 5 s after the latest activity. A click or key press counts as activity besides a cue: `gesture()` runs on every pointer-down and key-down, inside the gesture, so it wakes the device before the cue the click leads to, and that cue is not late. The cost: while the owner types or clicks, the device runs; it rests 5 s after they stop.
- A cue that has to wake a resting device waits at most 250 ms; the Web Audio spec keeps a resume that is not allowed to start pending until audio runs (not observed in WebKit itself), and without the limit such a cue would sound with the next click.
- The engine's own suspends are one field, `ownSuspend` (`until-wanted` / `until-activity` / none), and both kinds go through one `suspendWhenStill`; a system interruption clears it, so the waiting hint and dropped cues stay as they were.
- Checks: `pnpm typecheck`, `pnpm build`, `pnpm test` (see the PR); mutation checks on the rain, late-wake and interruption guards each turn a test red.

**2026-11-12 — two-axis review:** Standards found no hard violation. Its judgement calls were all taken: one suspend state instead of two flags, one shared suspend helper, one test `engine(wait)`, a cue-length-free assertion, and the Ticket 08 citation names its batch. Spec found nothing missing or out of scope, and two points were taken. First, an interruption during a rest made the next cue play late and hid the waiting hint; that is fixed and tested. Second, criterion 2 relies on `resume()` outside a gesture, so the native check now includes hearing the first cue after a rest. The review also noted that clicks count as activity, which is recorded above for the owner.
