# Won't Do is a kind of Completed, not a fourth work status

Status: accepted (2026-11-09)

The owner wanted to set aside Tasks that will not be done so they sink to the bottom like finished work. We considered a fourth work status shown after Completed, but chose to record Won't Do as a resolution of a Completed Task: the three work statuses and their fixed order (In Progress → Ready → Completed) stay unchanged, as ADR 0006 and ADR 0002 protect, and every interface keeps one notion of "done". The cost is one exception to "Completed means 100%": a Won't Do Task keeps the progress it had, so history stays truthful. Reopening clears Won't Do and returns the Task to the work status it had before it was set aside, with the progress it kept: a Task that had started stays In Progress even at 0% (ADR 0006), and only one that never started goes back to Ready.
