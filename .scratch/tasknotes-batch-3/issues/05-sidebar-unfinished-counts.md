# 05: Sidebar counts of unfinished Tasks

**What to build:** The number beside All tasks, Inbox and every List in the sidebar counts that place's unfinished Tasks (Ready and In Progress), shown on every folder, not only the open one; a Settings option controls it (spec, "Sidebar counts", Q45–Q46).

**Blocked by:** Ticket 04 (worked one at a time, owner 2026-11-11)

**Status:** needs-triage

**Implementation:** not started — the Settings option's exact form is asked when the ticket starts

## Acceptance criteria

- [ ] Each count is the core's number of that place's Ready and In Progress Tasks: Completed (Won't Do included) and Removed never count; the same rule for All tasks, Inbox and each List (and whether a List's count includes its sub-Lists is settled with the owner).
- [ ] Every folder shows its count, kept current after changes made here and from the CLI or MCP, without reading every Task.
- [ ] A Settings option controls the counts, in the form the owner chooses.
- [ ] Counts keep the folder's tabular tertiary style and WCAG AA on every sidebar fill.
- [ ] The owner checks it in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md`, ADRs and `AGENTS.md` before changing behavior.
- Own branch, Draft PR, `/tdd`, then `/code-review`; merge needs the owner's confirmation.
- The count rule lives in the core, shared by the desktop, CLI and MCP.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner request (Han), during Ticket 04:** "the number in the sidebar should not show the total Tasks, only the active (Ready) ones". Today the number is the Tasks loaded on the open page (`tasks.length`, "+" when more pages wait), shown on the open folder only, Completed included. Q45: count Ready and In Progress (the recommendation). Q46 (changed from the recommendation): every folder shows its count, but it can be set from Settings; whether that is an on/off switch or a choice of where counts show is asked when the ticket starts.
