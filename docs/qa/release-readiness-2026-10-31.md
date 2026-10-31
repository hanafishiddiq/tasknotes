# Tasknotes v1 release readiness, 2026-10-31

**Verdict: not ready yet.** Everything checkable without a person is checked. What remains needs the owner: the [click check](owner-check-2026-10-30.md), then the merge decision, then a verified backup before the real database is upgraded. Scope is local use on the owner's Mac. Signed or notarized public distribution is out of scope (spec) and is not implied here.

## Gates

| Gate | State | Evidence |
| --- | --- | --- |
| Search p95 under 250 ms at 50,000 Tasks and 10,000 Notes | Pass | 91 ms slowest through the app; [report](ticket-17-performance-2026-10-30.md) |
| Scrolling near 60 FPS at that size | Pass, scripted | 60 FPS, no frame over 20 ms; trackpad feel is in the click check |
| Paging keeps memory bounded | Pass | Largest answer 100 records; CLI 12 MB, MCP 17 MB |
| UI, CLI and MCP share invariants, audit, privacy, Removed and conflict rules | Pass, below the window | `crates/tasknotes-e2e/tests/scenario.rs`: real CLI and MCP binaries with the core's desktop store; UI behind a mocked seam in `src/*.test.tsx` |
| Crash and restart consistency | Pass | `crash.rs`: 24 rounds of `kill -9` on writers, plus kills during backup, restore and first run. Power loss is not covered |
| Corrupted backups refused; restore round trip | Pass | `recovery.rs`: seven kinds of bad backup, exact round trip through the real CLI |
| Restore never opens a private Note or undoes a decision | Pass | `restore_twice.rs`, `scenario.rs` |
| Upgrade of databases really made by old code | Pass | [migration.json](ticket-18-results/migration.json): commits at schema 1, 2, 3, 5 and 6, all records unchanged, schema identical to a new database. The owner's database is at schema 3 |
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
| 06 Notes | Draft PR #19 | Done | AI's native QA done; in the click check |
| 07–16 | Draft PR #20 | Done, 07, 11 and 14 re-reviewed | Open: click check |
| 17 Performance | Draft PR #20 | Done | Trackpad feel: click check |
| 18 Release quality | Draft PR #20 | Done | Open: click check |
| 19 Historic lag | Closed `wontfix` | | Root cause never proven; the trackpad step at full size settles it |

## Known exceptions

1. **Account name in the binary.** The release binary holds about 60 build paths under the builder's home folder, from Rust's panic messages. Harmless for local use. Remove with `--remap-path-prefix` before any public distribution.
2. **An email address in public history.** Seven merge commits made with GitHub's web button carry the owner's personal email in their metadata. Old commits cannot change without rewriting `main`. Turning on GitHub's "Keep my email addresses private" stops new ones.
3. **Not signed for distribution.** The bundle is signed ad hoc, which is enough for the owner's own Mac.
4. **A cold disk is not measured.** Emptying the OS file cache needs administrator rights; "fresh process" timings have a warm cache.
5. **A killed upgrade from an older schema** is not tested, only a killed first run. Each migration step is transactional, and the owner's upgrade happens after a verified backup.
6. **Windows and Linux** builds are not verified; the first milestone is macOS only.

## Before the real database is upgraded

The upgrade is one-way. In this order, each with the owner's approval:

1. Finish the click check and merge.
2. Copy `~/Library/Application Support/store.adamaja.tasknotes/` somewhere safe, and verify the copy opens.
3. Install the normal build.
4. Check the owner's Tasks are all there. If not, put the copy back and report.
