# Tasknotes

Language shared by the task manager, notes editor, desktop UI, and AI integration.

## Language

**Task**:
A unit of work with a title, optional description, work status, priority, and progress.

**List**:
An organizational collection of tasks. A task can be kept in the default Inbox when no other list is chosen.

**Inbox**:
The default list for tasks that have not been organized into another list.

**Ready**:
A Task that has not started in its present work cycle.
_Avoid_: Paused, Pending.

**In Progress**:
A Task that has been started but is not yet Completed, including when temporarily inactive.

**Completed**:
A task marked finished; its progress is 100%.

**Removed**:
A reversible, non-destructive disposition of a task, note, list or Note Folder, separate from a task's work status. Removing a list or folder hides what is inside it without changing their own disposition; Removed items are never purged automatically.
_Avoid_: Deleted, Permanently Deleted.

**Priority**:
One of five urgency/importance levels: Very Low, Low, Medium, High, Very High.

**Progress**:
A task's 0–100% completion indicator, accompanied by progress notes and a next step.

**Note**:
A standalone editable document that can optionally be linked to a task.

**Exclude from AI**:
A note-level privacy control that prevents access to that note via AI-facing interfaces.

**Progress Entry**:
An immutable dated record of a Task's progress update or work-status transition.

**Next Step**:
The next intended action recorded against a Task to help a person or AI resume work.

**Note Link**:
A reversible association between a Task and a Note; removing the association does not remove either entity.

**Note Folder**:
A category that organizes Notes independently from Task Lists. A Note Folder can contain other Note Folders, at most five levels deep, so they form a tree; sibling names are unique ignoring case.
_Avoid_: Folder alone where a Task List could be meant.

**Pinned**:
A Note kept above the other Notes in every Note list it appears in. Pinning, like moving a Note to another Note Folder, changes where it is shown, not what it says.

**Manual Order**:
The order a person gives Lists, and a folder's subfolders, by moving them. It is not a sort; a new one lands at the bottom.
_Avoid_: Sort order, Rank.

**Audit Entry**:
An immutable dated record that an official Tasknotes interface made or attempted a consequential change.

**Bulk Change**:
One request that removes, restores, moves or sets the status of several Tasks, or removes or restores several Notes; through an AI interface, also removing a List or Note Folder that holds anything. An AI interface can only ask for it: the request waits, with its preview, until the person approves or declines it in the desktop app, and the AI reads its status (pending, applied, declined, stale, expired). It applies all or nothing, and only while nothing it named (nor a move's target List) has changed; a request expires after a day.
_Avoid_: Batch, mass edit.

**Backup**:
A consistent local copy of all Tasknotes data, taken automatically once a local calendar day while Tasknotes runs (Daily), on request (Manual), and just before a restore (Pre-restore). Daily backups follow the 7 daily / 4 weekly / 3 monthly retention, the 10 newest Manual backups are kept, and Pre-restore backups are never removed automatically. Retention prunes backup copies only, never live Tasks, Notes or Lists.

**Backup Manifest**:
The record beside each Backup of when and why it was taken, its schema version, size and checksum; a restore refuses a Backup that does not match its manifest.

**Restore**:
Replacing all current data with a validated Backup, after stopping every other writer and keeping a Backup of the current data. The Audit Entries made since the Backup are kept, and the restore itself is recorded as one. A Note marked Exclude from AI now stays so, a Bulk Change in the Backup that the person decided since stays decided, and whatever changed since gets a new revision, so a client holding an old one is refused. Windows and commands that opened the data before must reload before they can change anything.
_Avoid_: Restore alone where restoring an item from Removed is meant.

**Export**:
A copy of Tasks and Notes written as JSON or Markdown for the person to keep or move elsewhere. An export made through an AI interface never contains Notes marked Exclude from AI.
