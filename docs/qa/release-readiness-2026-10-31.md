# Tasknotes v1 release readiness, 2026-10-31

**Verdict: approved for `main` on 2026-10-31.** The owner checked the app and approved the merge; see [the click check results](owner-check-results-2026-10-31.md). It was installed the same day; see [Installing over the real database](#installing-over-the-real-database). Keyboard navigation, VoiceOver, Reduce motion and going offline were not all confirmed one by one.

## Gates

| Gate | State | Evidence |
| --- | --- | --- |
| Search p95 under 250 ms at 50,000 Tasks and 10,000 Notes | Pass | 91 ms slowest through the app; [report](ticket-17-performance-2026-10-30.md) |
| Scrolling near 60 FPS at that size | Pass after a fix | A real scroll measured about 53 FPS against 58 for a plain page, and the owner felt it. With all rows kept mounted it measures 58 to 59 FPS in the same view; see [the report](ticket-17-performance-2026-10-30.md) |
| Paging keeps memory bounded | Pass | Largest answer 100 records; CLI 12 MB, MCP 17 MB |
| UI, CLI and MCP share invariants, audit, privacy, Removed and conflict rules | Pass, below the window | `crates/tasknotes-e2e/tests/scenario.rs`: real CLI and MCP binaries with the core's desktop store; UI behind a mocked seam in `src/*.test.tsx` |
| Crash and restart consistency | Pass | `crash.rs`: 24 rounds of `kill -9` on writers, plus kills during backup, restore and first run. Power loss is not covered |
| Corrupted backups refused; restore round trip | Pass | `recovery.rs`: seven kinds of bad backup, exact round trip through the real CLI |
| Restore never opens a private Note, reuses a revision or undoes a decision | Pass | `restore_twice.rs`, `scenario.rs`, and an independent model test: 2,000 random operation sequences, 21,921 restores, no violation. The same test broke the earlier design |
| Upgrade of databases really made by old code | Pass | [migration.json](ticket-18-results/migration.json): commits at schema 1, 2, 3, 5 and 6, upgraded to schema 15, all records unchanged, schema identical to a new database. The owner's database was at schema 3 |
| Processes opening one database at once | Pass | `stores_opening_at_the_same_moment_all_succeed` |
| Accessibility, automated | Pass | axe over every view in both themes; contrast from the stylesheet at AA; keyboard reach and focus return |
| Accessibility, by a person | **Open** | VoiceOver, visible focus rings, Reduce Motion, on-screen contrast: click check |
| Packaged app: first launch, persistence, no network sockets | Pass | [package-smoke.json](ticket-18-results/package-smoke.json), QA identifier, without a screen |
| Packaged app: keyboard capture, menu bar, audio, Notes on screen | **Open** | Click check |
| Audio CPU | **Open** | Measured by the owner in the click check; the pre-rendered rain has only been measured in a browser (Ticket 16) |
| Public repository audit | Pass, one note | [repo-audit.json](ticket-18-results/repo-audit.json): no database, backup, audio or secret in any commit; patterns proven on planted fakes |
| Release bundle audit | Pass, one note | No data, audio or measuring probe in the bundle |
| A real MCP client session | **Open** | Protocol tested against the real binary with a test client only |

## Tickets

| Ticket | Merged to `main` | Review | Native check by the owner |
| --- | --- | --- | --- |
| 01–05 | Yes | Done | Done |
| 06 Notes | Yes | Done | Click check |
| 07–16 | Yes | Done, 07, 11 and 14 re-reviewed | Click check |
| 17 Performance | Yes | Done | Owner judged scrolling smooth after the fix |
| 18 Release quality | Yes | Done | Click check |
| 19 Historic lag | Closed `wontfix` | | Root cause never proven; the trackpad step at full size settles it |

## Known exceptions

1. **Account name in the binary.** The release binary holds about 60 build paths under the builder's home folder, from Rust's panic messages. Harmless for local use. Remove with `--remap-path-prefix` before any public distribution.
2. **An email address in public history.** Seven merge commits made with GitHub's web button carry the owner's personal email in their metadata. Old commits cannot change without rewriting `main`. Turning on GitHub's "Keep my email addresses private" stops new ones.
3. **Not signed for distribution.** The bundle is signed ad hoc, which is enough for the owner's own Mac.
4. **A cold disk is not measured.** Emptying the OS file cache needs administrator rights; "fresh process" timings have a warm cache.
5. **A killed upgrade from an older schema** is not tested, only a killed first run. Each migration step is transactional, and the owner's upgrade happens after a verified backup.
6. **Windows and Linux** builds are not verified; the first milestone is macOS only.
7. **Restoring the same content twice moves its revision.** A client then gets a false `revision_conflict` and reads again, and a request waiting on that row goes stale, so the AI asks again. This fails safe.

## Installing over the real database

The upgrade is one-way, so it ran in this order on 2026-10-31, with the owner's approval:

1. The click check finished and the merge landed.
2. `~/Library/Application Support/store.adamaja.tasknotes/` was copied somewhere safe. The copy matched by checksum and passed `integrity_check`: schema 3, 0 Tasks, 0 Progress Entries.
3. The normal build was installed. Its code matches `main` at `f3a0227`; the signature is ad hoc, so `spctl` rejects it (exception 3).
4. The app opened with its empty list and no network sockets. A copy of the database read schema 15, `integrity_check` ok, 0 Tasks, 0 Notes, and a daily backup exists.

The app was reinstalled that day with its own icon and menu bar image, ahead of `main`. The database file's checksum did not change across the swap. macOS kept showing the old icon in the Dock until the Dock's icon cache was set aside and the Dock restarted.

The database held nothing to lose, so this did not exercise a real upgrade with data; the [migration rehearsal](ticket-18-results/migration.json) covers that.
