# Manual order for Lists and Note Folders

**Status:** ready-for-human
**Blocked by:** None
**Implementation:** in progress (Draft PR open; waiting for the owner's check and approval to merge)

## What the owner asked for (2026-10-31)

Arrange Lists and Note Folders in the sidebar by drag and drop, and keep that order after closing the app.

## Decisions

Recorded in `docs/adr/0007-manual-sidebar-order.md`: dragging a folder can change its parent, and a new List or folder lands at the bottom.

## Acceptance

- Dragging changes the order of Lists and Note Folders, and the order survives reopening.
- Alt with the arrow keys does the same from the keyboard.
- Upgrading from schema 15 keeps the order the sidebar showed.
- Desktop, CLI and MCP share the same write, with revision and conflict checks.

## When it is installed

Schema 16 is refused by older binaries. The app, `~/.local/bin/tasknotes` and `~/.local/bin/tasknotes-mcp` have to be updated together, and running MCP servers restarted.
