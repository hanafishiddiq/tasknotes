# Lists nest like Note Folders, only to group

Status: accepted (2026-11-09)

Lists were flat. The owner wanted to group Lists the way Note Folders group Notes, so a List may now sit inside another, up to the same five levels as Note Folders, with one rule for both trees. A List shows only its own Tasks: nesting groups Lists and never rolls Tasks up, because a roll-up would make a parent's Custom Order, counts and removal ambiguous. Inbox stays at the top level and holds no Lists, since it is where unsorted capture lands. Removing a List that holds Lists follows the existing container rule (ADR 0002): what is inside is hidden, not changed, and comes back with it.
