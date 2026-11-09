# 01: Icons and colours for Lists and Note Folders

**What to build:** Let the owner give each List and Note Folder an icon and a colour (spec stories 15).

**Blocked by:** 04 (manual sidebar order, PR #24), 11 — Sidebar hover, width and action icons

**Status:** ready-for-agent

**Implementation:** not started

## Acceptance criteria

- [ ] Each List and Note Folder can have one icon and one of 12 colours; both optional, default unchanged look.
- [ ] The 12 colours meet AA contrast as icon colour on the sidebar in light and dark themes.
- [ ] The picker offers the app's Lucide icons with a search box; choosing is keyboard accessible.
- [ ] Emoji already in names are not changed or stripped.
- [ ] CLI and MCP can read and set icon and colour with revision checks; exports include them.

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `AGENTS.md` and the ADRs it names before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; no merge to `main` without the owner's confirmation.
- Business rules live in the shared core; the UI, CLI and MCP call them.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-09 — owner decision (Han):** in batch 2, folded together with the sidebar work. Lists and Note Folders both, fixed palette of 12, Lucide with search, CLI/MCP included, emoji names untouched.
