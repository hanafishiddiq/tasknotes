# Removed is a reversible disposition, not a work status

Status: accepted (2026-10-11)

Every task retains one of three work statuses (In Progress, Ready, Completed) even when it is in Removed. Notes and lists can also be Removed, and ordinary deletion never physically destroys user data; restoring an item recovers its previous state. We chose a separate reversible disposition over a fourth status or destructive deletion so that fixed task ordering, progress history, and restoration remain consistent. Removing a list hides its contents from active views without bulk-removing each child, preserving their individual state. 

**Containers (owner decision 2026-10-25, Ticket 09).** Lists and Note Folders follow one rule. Removing a container hides everything in it (for a folder, its whole subtree of subfolders and Notes) without changing any child's own Removed disposition. Restoring the container brings those children back, except a child that had been moved to Removed on its own, which stays in Removed. While a container is in Removed, a child it hides can be neither removed nor restored on its own; the container comes first.

**Approved by the owner on 2026-10-26:**

- An item in Removed, or hidden by a Removed container, is read-only until restored: editing, moving, pinning, linking and creating inside it are refused, and so is bringing back an unlinked link to it. Unlinking stays allowed.
- Restoring into a container that is still in Removed is refused ("restore the List / folder first"); nothing is silently restored into Inbox or the top level.
- A Removed folder frees its name, as a Removed List does; restoring it over an active sibling with the same name is refused.
