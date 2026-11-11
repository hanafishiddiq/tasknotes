# Started Tasks remain In Progress while idle

Status: accepted (2026-10-11)

Ready means work has not begun; In Progress means work has begun but has not been completed, whether or not work is actively happening at this moment. We deliberately do not add a Paused status or automatically move inactive Tasks back to Ready, because those interpretations confuse task lifecycle with momentary activity and would break the user's always-on-top ordering of started work. Progress, Next Step, and dated history stay intact through idle periods. A future explicit reset/start-over workflow must be deliberate and cannot silently erase history.

## Addendum (2026-11-11): reopening returns a Task to where it was

Owner decision Q48. Reopening a Completed Task, through its check, the check's Undo, the status control, the CLI or MCP, returns it to the work status it had before it was completed: a Task completed from Ready goes back to Ready at 0%, one completed from In Progress goes back to In Progress with its previous incomplete progress. This keeps the rule above rather than bending it: Ready is still only for work that has not started, and a Task completed straight from Ready had not. Lowering such a Task's progress to 0% lands on Ready too; any other percentage starts it.

It is one core rule for every interface (ADR 0001), the one Won't Do already follows with `wont_do_from` (ADR 0008): a Completed Task is treated as the Task it was before. The core reads where it came from in the Progress Entry that completed it, the latest one whose previous status is not Completed (every status change writes one, and none leaves Completed without one), so the schema does not change; Tasks carry it as `completedFrom` (desktop) and `completed_from` (CLI and MCP), and the status control offers Ready only when it is Ready. Choosing In Progress explicitly still starts the Task, at its previous incomplete progress, as it does for a Won't Do Task. A Task completed before Progress Entries existed (schema v3) has no such entry and keeps the earlier rule: it counts as started, so it reopens In Progress.
