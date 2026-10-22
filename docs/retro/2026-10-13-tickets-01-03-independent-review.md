# Retro — Tasknotes Tickets 01–03 independent code review

Date: 2026-10-13. Reference: [fixed-point two-axis review](../reviews/tickets-01-03-independent-2026-10-13.md). This is a tooling/process retrospective, not a product-code change.

## Primary observations → future agent environment improvements

1. **Agent availability should be checked, not assumed.** Previously the allowed `agy` reviewer was described as unavailable. This session located `~/.local/bin/agy` and proved a read-only smoke test could read `AGENTS.md`. For future `/code-review`, first inspect the CLI entrypoint and supported models in a controlled read-only invocation. This is a **navigation** and **tool-economy** lesson; don't infer reviewer availability from an older session.

2. **Headless review needed a least-privilege source packet.** The agent's `command` permission was denied in sandbox plan mode, and an initial `--effort` flag was unsupported by the Claude model. Instead of elevating permissions, the host collected exact immutable Git diffs and relevant public standards/spec texts, then invoked separate reviewers with self-contained, tool-free prompts. Six reviewers completed, all with exit 0 and empty error output. **Repeatable technique:** pin SHA → materialize public source packet → assert no personal data → run separate Standards/Spec read-only AI agents → capture raw outputs → verify their factual claims. Do not use `--dangerously-skip-permissions` for routine read-only code review.

3. **Independent agents can still misread the boundaries between tickets.** A Ticket 01 Spec review misclassified its initial Ready/Medium-only display as a blocker, although changing priority and status belongs to Ticket 02 and was delivered there. A Ticket 03 Spec review missed a later exact Progress Note SQLite assertion despite receiving the accepted native QA evidence. **Guardrail:** verify every reported blocker against the originating *ticket's* scope, its exact head SHA, and subsequent evidence before editing code or changing tracker status. Archive the original review unchanged and add factual adjudication rather than silently rewriting history.

4. **Stacked PR safety is a distinct review dimension.** Issue19's correct closure documentation exists in Draft #13 based on the *abandoned QA experimental stack*. Merging that stack to carry documentation would accidentally integrate irrelevant QA code. Future reviews should inspect the PR **base chain** before issuing merge readiness, and cherry-pick only intentional documentation into a reviewed product-derived branch after owner authorization. See [merge-readiness plan](../reviews/tickets-01-03-merge-readiness-2026-10-13.md).

## Completion criteria

- Existing CI already runs React tests, typecheck, build, Rust core tests, fmt and clippy; no duplicate automated check was added.
- Review artifacts now separate external-agent opinions from the implementing assistant's fact checks, and distinguish **independent AI review** from authenticated GitHub human approval.
- Original Tasknotes app and personal SQLite remained untouched; features and Issue19 QA experiments were not merged. Future review work can use the same fixed-point, least-privilege method without repeating the old permission dead end.
