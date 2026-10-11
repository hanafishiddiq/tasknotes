# Domain documentation

This project uses a single domain context during planning.

Before adding features or tests:
1. Read root `GLOSSARY.md` for canonical task/notes terms and avoidlisted synonyms.
2. Read relevant decisions in `docs/adr/`; note contradictions explicitly.
3. Read `docs/planning/DECISIONS.md` for already-approved requirements.

The `/domain-modeling` skill updates the glossary when a term becomes precise and creates an ADR only for hard-to-reverse, surprising, genuine trade-offs. Keep implementation-specific details out of the glossary. Do not create a multi-context map without evidence that distinct domains genuinely require it.
