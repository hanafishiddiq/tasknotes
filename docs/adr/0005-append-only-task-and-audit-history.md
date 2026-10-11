# Append-only progress and change history

Status: accepted (2026-10-11)

Task progress entries and meaningful mutation audit entries are written as append-only history, not overwritten in place. The user chose traceability over a simpler mutable current-state-only model because tasks may be changed by both AI tools and the desktop UI and progress must be understandable after multiple sessions. Corrections are represented by new entries and current-state projections; the interface should offer concise recent history without loading the entire log. Retention, recovery, and storage pressure must be tested without silently deleting user task data.
