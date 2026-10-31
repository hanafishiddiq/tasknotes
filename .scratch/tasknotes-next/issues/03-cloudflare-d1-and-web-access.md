# Cloudflare D1 database and web access with login

**Status:** needs-triage
**Blocked by:** None
**Implementation:** not started

## What the owner asked for (2026-10-31)

The owner wants the database to move to Cloudflare D1, so Tasknotes also opens from a website with a login, with no app installed.

## Questions to settle first

- This reverses earlier decisions: the v1 spec lists cloud accounts, sync and a hosted API as out of scope, and ADR 0001 makes Tasknotes local-first. It needs its own ADR before any code.
- Does the desktop app still work offline? If so, how do offline edits merge with web edits, given the revision checks?
- Who holds the private data: can Cloudflare read Tasks and Notes, and what happens to Exclude from AI on the server?
- Which login, and how is an account recovered?
- How do Backups, Restore, Audit Entries, Quick Capture, CLI and MCP work when the data is remote?
- How does the owner's existing local data move across, and can that be reversed?

Not part of Tasknotes v1. Run `/grill-with-docs` before writing a spec.
