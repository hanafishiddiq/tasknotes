# Ticket 04 /retro — targeted process lessons

Date: 2026-10-13. This is process feedback, not a feature requirement.

1. **Acceptance coverage must include actual viewport bounds.** Database pagination was already limited but React kept appending DOM rows. A deterministic 120-Task contract test proved the mismatch. Preserve separate tests of SQL bounded pages AND rendered Task card count; a green SQL cursor test cannot prove UI virtualization.
2. **Failure-path forms need write-confirmed completion.** List create/rename cleared the draft even when the async Tauri write rejected. Contract tests now assert the input survives conflict, and form callbacks return write success. This is a test seam at user-visible interface.
3. **Independent review is a capacity-bound external dependency.** Separate AGY reviewers were dispatched but both reported RESOURCE_EXHAUSTED 429 with empty results. A self-review is useful but should be labelled explicitly, and an out-of-credits agent must never be represented as independent approval. Preserved read-only source packets avoid redoing diff collection when a real independent reviewer becomes available.
4. **Visible native UI is not proof of interactive acceptance.** The isolated QA app rendered on Mac, but AX enumerated only native window chrome and no inner WKWebView controls; avoid coordinate injection or user-app focus workarounds. Keep owner app running; request a legitimate human native QA pass if final native acceptance remains a requirement.

Existing CI already runs Rust core, formatting, Clippy, React and typecheck. No duplicate CI scripts or environment changes are needed merely to encode these lessons.
