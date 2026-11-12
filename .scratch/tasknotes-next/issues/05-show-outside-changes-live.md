# 05: Show CLI and MCP changes without reopening

**What to build:** Already implemented in Draft PR #23 (`claude/youthful-elion-4ee635`): the window picks up Tasks, Lists, Notes and Note Folders changed by the CLI or MCP, keeping focus, scroll and unsaved drafts.

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** completed — PR #23 merged 2026-11-09; checked on the installed build 2026-11-12

## Acceptance criteria

- [x] Independent review of PR #23 with findings addressed. *(standards + spec review 2026-11-09; fixes 0ed8660…c7e247e, Vitest 980, Rust green; combined with #22/#24/#25: Vitest 999, Rust core 239 / CLI 46 / MCP 35 / app 42)*
- [x] A change made through the CLI appears in the open window within a few seconds without a reopen, on the real build. _Evidence: 2026-11-12 comment: a CLI-created Task was in the open Inbox about 4–5 s later._
- [x] A Task with unsaved edits stays visible after such a reload until saved or discarded. _Evidence: same check: the open Task kept its unsaved title and its Save button through the reload._

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09:** recorded as a batch-2 ticket so the single build includes it. Implementation status: in progress — awaiting review.

**2026-11-12 — native check on the installed build** (the agent through Cua, owner-approved, synthetic test Tasks in the owner's store): with the Inbox open and a Task's title edited but not saved ("… draft"), `tasknotes task create --title …` added a Task at 10:03:53. A read of the window about 2 s later did not show it yet; a read 4–5 s after the create did, with no reopen. The edited Task stayed open with its unsaved title and "Save changes". The edit was then undone by hand and both test Tasks moved to Removed.
