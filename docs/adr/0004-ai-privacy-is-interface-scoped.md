# AI privacy controls are enforced at official interfaces

Status: accepted (2026-10-11)

Notes marked Exclude from AI must not be readable, searchable, summarized, or modified through official Tasknotes AI interfaces. This choice intentionally creates an interface-level privacy boundary rather than promising protection from software that has arbitrary filesystem or shell access to the user's Mac. The shared application core will enforce that policy across CLI and MCP operations, while the desktop UI remains authorized to show the note to its human user. Strong protection against a fully privileged local process would require additional OS-level isolation or encryption outside the first release.
