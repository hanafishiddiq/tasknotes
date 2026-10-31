# Lists and Note Folders keep a manual order

Status: accepted (2026-10-31, owner decisions)

Lists used to show newest first and Note Folders by name, so a person could not arrange either. Both now keep the order the person gives them, stored in the shared core as an integer `position` among siblings: all Lists are one group, and each folder's children (or the top level) are another. Schema v16 starts from what the sidebar showed, so nothing appears to move on upgrade.

**Owner decisions.** Dragging a folder can also change its parent, as in Apple Notes: it can land inside another folder or between folders at another level. A new List or folder lands at the bottom, right above the field that adds it, and a folder moved with the Move control lands at the bottom of its new parent. A new name no longer changes a folder's place.

**The write.** One operation places one item directly after a named sibling, or first: `reorder_list` and `place_note_folder`, shared by the desktop, the CLI and MCP. A relative anchor means the sidebar never has to load every page, and a change of parent and place happen in one transaction. It checks the moved item's revision and bumps only that item's; neighbours keep theirs, so a rename open on another row never conflicts with something moving past it. An anchor that is gone, in Removed or under another parent is `revision_conflict`: the caller's view is out of date and it should read again.

**Removed.** A List or folder in Removed keeps its position, so restoring it brings it back where it was. A move that changes nothing a person can see is `unchanged`.

We chose whole integers renumbered on a move over fractional positions because sibling groups are small, and over a separate order table because position has no meaning apart from its row. Moving through an AI interface is a single-item change, so it does not go through Bulk Change approval.
