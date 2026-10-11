# Issue tracker: Local Markdown

Tasknotes records specs and implementation tickets as versioned Markdown under `.scratch/`. GitHub hosts the public source repository, **not** the authoritative task tracker for this project.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`.
- Feature spec: `.scratch/<feature-slug>/spec.md`.
- One issue per file: `.scratch/<feature-slug>/issues/<NN>-<slug>.md`.
- Assign numbers in dependency order, starting at 01.
- Each issue includes a `Blocked by:` field and a `Status:` field using `docs/agents/triage-labels.md`.
- Use a `## Comments` section for additional decisions and discussion when needed.

## Skill operations

- **Publish spec:** create the feature's `spec.md`.
- **Publish issue:** create a numbered issue file; never collapse separate vertical slices into a single file.
- **Fetch:** read the referenced file(s) and their explicit blockers.
- **Select work:** choose a ready ticket whose blockers are finished, rather than jumping ahead.
- **Close:** record acceptance-test evidence and the completed status in the corresponding issue.

## Wayfinder compatibility

When using `/wayfinder`, create `.scratch/<effort>/map.md` and `.scratch/<effort>/issues/NN-<slug>.md`. Every map child records `Type:`, `Status: claimed|resolved`, and `Blocked by:`; resolve decisions into the map with pointers to supporting evidence.

Keep personal task data and other private runtime assets outside `.scratch/` and out of the public repository.
