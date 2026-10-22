---
version: 1
slug: "src-app-tsx"
primary_target: "src/App.tsx"
related_targets: ["src/components/TaskRow.tsx","src/components/TaskSidebar.tsx","src/App.css"]
---

Scope: Tasknotes desktop task workspace (All tasks / Inbox / List views). Mode: Operate.
Audience/job: one developer, window open all day beside editor; capture, scan In Progress, resume via Next Step.
Constraints: fixed groups In Progress → Ready → Completed; priority label + icon; light and dark themes (dark default); details expand inline one at a time (spec stories 8–10); no macOS-only window chrome.

## Direction contract

THESIS: Tasknotes wears Apple Notes' design language, played straight (owner-pinned canon). It refuses the lavender SaaS dashboard: no hero, eyebrow, cards, gradients, badges.
OWN-WORLD: Flat Notes grays (#1e1e1e content, #282828 sidebar, #353535 hairlines), Notes yellow #ffd60a only for folder icons, status rings, tags, primary action and the selected row (muted gold #67551a). System font 13px, bold titles, gray second lines, inset hairline separators. Light theme mirrors it on white with #f0f0ef sidebar.
STORY: The user sees what is In Progress and its percent and Next Step at a glance, types one field to capture, opens one row to edit like a note page.
FIRST VIEWPORT: 210px folder sidebar (Tasknotes: All tasks, Inbox; Lists with yellow folders; New List; appearance toggle at bottom). Thin toolbar with sort and compose icon. Centered 720px column: 22px bold page title + count, dashed-ring "New task" row, bold group headings, Notes list cells (bold title; "60%  Next: …" gray line; priority/deadline/tags right).
FORM: Category canon (standing exit) on owner request, Apple Notes as the named bar; seed b0568480.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
