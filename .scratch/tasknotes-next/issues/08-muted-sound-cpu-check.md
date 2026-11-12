# 08: Measure what muted sound costs

**What to build:** Measure CPU of the real app in three states and report (spec story 23).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** completed — measured 2026-11-12 in the installed app; the owner chose the proposed fix, built as Ticket 17 (PR #48)

## Acceptance criteria

- [x] CPU of the Tasknotes processes (including WebKit audio XPC) is sampled for the same window of time with sound on, only rain muted, and rain plus interaction sounds muted, on a synthetic store with the screen awake. _Measured in the installed app on the owner's store (the ticket's "real app"; owner-approved), idle on a List view, screen awake; see the 2026-11-12 comment._
- [x] Results and method are recorded here; if "only rain muted" keeps an idle audio device measurably busy, a fix is proposed to the owner before any change. _It does (about 1.3 % of one core in WebKit's audio process); the proposal is below and waits for the owner._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09:** code reading shows muting rain stops its sources and that the audio device is suspended only when both channels are muted (`src/lib/sound/sound-engine.ts`, `quiet()`); CPU was never measured on the native app.

**2026-11-12 — measured in the installed app (owner-approved; settings changed by the agent through Cua and restored afterwards):**

Method: CPU seconds from `ps -o time=` taken 60 s apart for the Tasknotes app process, its WebKit GPU process (which renders Web Audio), WebContent and Networking processes, and `coreaudiod`; one 60 s window per state, Tasknotes idle on a List view and not clicked during the window; the screen was awake and unlocked at both ends of every window (checked with `CGDisplayIsAsleep` and the `CGSSessionScreenIsLocked` flag of `CGSessionCopyCurrentDictionary`). States were set in Settings: interaction sounds 12 % and rain 20 % (sound on); interaction sounds 12 % with rain muted (only rain muted, the owner's own setting); both muted.

| State | Tasknotes total | WebKit GPU (audio) | WebContent | App |
|---|---|---|---|---|
| Sound on | 1.73 % | 0.43 % | 1.13 % | 0.16 % |
| Only rain muted (run 1 / run 2) | 2.35 % / 1.98 % | 1.26 % / 1.26 % | 0.90 % / 0.58 % | 0.18 % / 0.13 % |
| Both muted (run 1 / run 2) | 0.21 % / 0.21 % | 0.00 % / 0.00 % | 0.08 % / 0.10 % | 0.13 % / 0.11 % |

Percentages are of one core. `coreaudiod` used 5.0–7.2 % in every state; it serves every app on the Mac, so it is not attributed to Tasknotes. The Networking process used nothing.

Reading: with only rain muted, the audio device stays running (the interaction channel is still audible, so `sync()` never reaches `quiet()`), and WebKit's audio process keeps rendering silence: about 1.3 % of one core, about ten times the both-muted total. Sound on measured lower in the GPU process than only-rain-muted in its single run; that is unexplained and was not repeated.

Proposal to the owner (not built): suspend the audio device whenever no rain is playing and no interaction cue has played for a few seconds, and resume it for the next cue. Cost: the first click after a quiet spell may start a few milliseconds late. Alternative: keep it as is (about 2 % of one core while the app is open with rain muted).

**2026-11-12 — owner's decision:** "Ya, perbaiki": the audio device rests between cues when no rain plays; built as Ticket 17 (`17-rest-audio-between-cues.md`, Draft PR #48).
