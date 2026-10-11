# Removed is a reversible disposition, not a work status

Status: accepted (2026-10-11)

Every task retains one of three work statuses (In Progress, Ready, Completed) even when it is in Removed. Notes and lists can also be Removed, and ordinary deletion never physically destroys user data; restoring an item recovers its previous state. We chose a separate reversible disposition over a fourth status or destructive deletion so that fixed task ordering, progress history, and restoration remain consistent. Removing a list hides its contents from active views without bulk-removing each child, preserving their individual state. The precise behavior of independently removed children within a removed list remains a planning edge case.
