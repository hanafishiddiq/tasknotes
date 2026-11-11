# 03: Task right-click menu

**What to build:** Right-clicking a Task row (or Shift+F10 / the menu key on it) opens a small menu in the app's own look: Open or Close; Ready, In Progress, Completed, Won't Do; a Priority submenu; Move to Removed (spec, "Task right-click menu", Q20–Q24).

**Blocked by:** None

**Status:** ready-for-agent

**Implementation:** merged to main in PR #40 (`d9f0c22`); the owner's native check waits for the batch-3 install build

## Acceptance criteria

- [x] Right-clicking a Task's row opens the menu at the pointer instead of the system menu; inside an open Task's fields the system menu (copy, paste) stays. — `src/task-context-menu.test.tsx` ("opens on a right-click…", "leaves the system menu inside an open Task's fields").
- [x] The status items tick the current choice and dim the ones the status control refuses (Ready once work started, Won't do on a finished Task, with the same reason), through one shared rule. — `statusChoiceState` in `src/lib/task-visuals.ts`, used by the status select and the menu; test "dims Ready once work has started…".
- [x] Completed on a Ready or In Progress Task does what the round check does (completion sound, Undo bar); the other status choices and Priority do what their controls do; Move to Removed does what the red button does (Undo notice). — tests "completes a Task as the round check does", "sets Won't do…", "changes the priority from its submenu…", "moves a Task to Removed…". On a Won't do Task, Completed is the status control's change (the round check there clears Won't do).
- [x] While the Task has unsaved edits or a save is on its way, the choices that change it are dimmed; Open stays. — test "dims every change while the Task has unsaved edits…" (a click on a dimmed Move to Removed does nothing).
- [x] Keyboard: Shift+F10 or the menu key on a row opens it; arrows move between the enabled items, Home and End jump, Return or Space chooses, Right opens Priority and Left closes it, Escape closes and puts focus back where it was, Tab closes. — tests "opens from the keyboard…", "moves with the arrows…", "changes the priority…", "closes on … Tab".
- [x] It closes on a choice, a click elsewhere, scrolling, resizing or the window losing focus; it never leaves the window, and Priority opens to the left when there is no room on the right. — tests "closes on a click elsewhere…", "stays inside the window near its edges"; the Priority sheet is placed once on screen with its real size, flipped left without room and kept above the window's bottom (test "places Priority beside its item…").
- [x] Its look follows DESIGN.md (sidebar gray sheet, hairline, 8px radius, no shadow; highlighted item in the selected-row colour; Move to Removed in Alert Red) and every text on it keeps WCAG AA. — stylesheet test in `src/task-context-menu.test.tsx`; `src/a11y-contrast.test.tsx` (`.task-menu-item:focus` on the selected-row colour, Remove on the sidebar and highlighted on solid red). A red tint as Remove's highlight measured 4.05:1 in light, so the highlight is solid red with canvas-coloured text; the focus replacement is registered in `src/stylesheet-a11y.test.tsx`.
- [ ] The owner checks it in the installed app (batch-3 click check).

## Delivery guardrails

- Read `../spec.md`, `GLOSSARY.md`, `DESIGN.md` and `AGENTS.md` before changing behavior.
- Own branch and worktree, Draft PR, `/tdd`, then `/code-review`; merge after green CI (owner Q24).
- Business rules stay in the core and the shared helpers; the menu calls the same changes as the controls.
- Synthetic fixtures only; never touch the owner's real store.

## Comments

**2026-11-11 — owner decisions (Han), Q20–Q24:** see the spec. Q21 chose a custom menu over the macOS one so it matches the app's design.

**2026-11-11 — review on PR #40:** fixed: the Priority sheet used a guessed 160px width before it was on screen and had no bottom limit, so it could leave the window; the menu's state now clears when its Task leaves the page or the Task view is left, so it never comes back with the Task; any ⌘ or Ctrl shortcut closes it and then does its own work; a choice gives focus back where it was (the row's button when opened from the keyboard); Priority stays open 250 ms while the pointer crosses another item toward it; `toggleTask` serves the row and the menu; completing through the round check reads `checkActionOf`; the menu takes one `MenuPoint`; the sheets share one key handler; named constants replace bare numbers. Technical call: Priority lists Very low → Very high, the open Task's Priority order. Kept: the menu dims changes by the round check's rule (stricter than the status control), as Q22 asked.
