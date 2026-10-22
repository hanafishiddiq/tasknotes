# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Rendered in a Tauri v2 WebView. MVP ships on macOS; the owner intends later use on other devices (which platforms, and whether data moves between them, is undecided). Design must not depend on macOS-only chrome to make sense.

## Users

One person (the owner) in the MVP: a developer who keeps Tasknotes open all day beside an editor and terminal, while local AI agents read and write the same Tasks through CLI/MCP.

## Product Purpose

A fast, local-first place for many personal Tasks and Notes where progress is never opaque. Success: the owner can see what is In Progress, what to do next, and what changed, at a glance, with tens of thousands of items, without the UI or an AI agent silently corrupting data.

## Positioning

Progress is first-class (percentage, dated Progress Entries, Next Step), and humans and AI agents operate through one shared core with revision checks and an audit trail, rather than AI scraping a UI.

## Operating Context

- Window stays open for the whole working day, usually not full-screen, next to developer tools.
- AI agents edit concurrently; the UI must show current state and surface revision conflicts plainly.
- Offline, no account, no cloud in v1.

## Capabilities and Constraints

- Navigation: All Tasks (default), Inbox, Lists, Notes, Removed, Settings.
- Task: title, description, List, tags, optional date deadline with overdue indicator, work status, five priorities (Very Low…Very High, Medium default), progress 0–100, Next Step, Progress Entries.
- Fixed status group order In Progress → Ready → Completed under every sort. Removed is a separate reversible disposition, never "deleted".
- Priority must be shown with label and symbol, not color alone.
- Light and dark themes are both required (spec story 38); owner chose dark as the default.
- Keyboard navigation, Cmd+K, clear focus, Undo, Reduce Motion; subtle motion only.
- Paginated/virtualized lists; near-60 FPS scrolling target.
- Notes (rich text over canonical Markdown), CLI/MCP, sound, quick capture are later tickets.

## Brand Commitments

Name "Tasknotes". Owner's direction for the UI: modern, lightweight, simple, dark-first.

Standing reference (owner, 2026-10-22): the look and color theme of Apple Notes on macOS is the bar — "Simple, Modern, Lightweight". Follow its restraint and palette at full craft; do not imitate macOS-only window chrome, since other devices are planned. Owner explicitly wants to avoid busy/colorful UI and template-generic SaaS dashboards.

## Evidence on Hand

Synthetic fixtures only. The repository is public: no personal Tasks, Notes, screenshots of real data, or fabricated usage claims.

## Product Principles

1. Calm over busy: it is on screen all day, so nothing competes for attention without reason.
2. State is truthful: status, progress, overdue and conflicts are shown exactly as the core reports them.
3. Fast to capture, fast to scan: the common path is a single field and a glance.
4. Reversible by default: Removed and Undo, never destructive surprises.

## Accessibility & Inclusion

Readable contrast in both themes, visible focus, full keyboard operation, Reduce Motion respected, no color-only meaning.
