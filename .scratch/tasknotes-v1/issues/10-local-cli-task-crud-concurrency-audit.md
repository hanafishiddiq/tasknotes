# 10: Local CLI Task CRUD with concurrency and audit

**What to build:** Structured JSON CLI Tasks CRUD, filtering, cursor pagination, revision-protected edits, stable error codes, and append-only Audit Entries. Operate while the desktop window is closed; race UI and CLI writers to prove no silent overwrite.

**Blocked by:** 03 — Progress history and Next Step, 05 — Indexed Task search and filters, 09 — Removed and restore

**Status:** ready-for-agent

**Implementation:** implemented, awaiting independent review (branch `feat/ticket-10-local-cli`, plan `docs/plans/ticket-10-local-cli-2026-10-26.md`).

## Acceptance criteria

- [x] A trusted local CLI can create, read, update, query, remove and restore Tasks and manage Lists through the Rust core without opening the desktop window.
- [x] AI-facing CLI capabilities require explicit local trust configuration, with safe refusal of unconfigured use. This is an interface authorization boundary, not a promise of defense against processes with unrestricted same-user filesystem access.
- [x] Machine output uses a documented JSON schema version, stable IDs, revision metadata, 50-item default page size and next_cursor when applicable.
- [x] Concurrent UI and CLI updates using an obsolete revision fail with a stable conflict error instead of overwriting the winner.
- [x] Successful mutations and meaningful rejected mutations are reflected in append-only Audit Entries without leaking secrets.
- [x] Malformed CLI inputs, pagination cursors and transaction failures are rejected safely with stable nonzero exit codes; no listening HTTP port is required.

## Verification

- [x] Run separate-process CLI contract tests and a concurrent UI-versus-CLI write race using temporary SQLite.
- [x] Relevant tests pass and evidence is recorded here.
- [ ] Run `/code-review` and address findings before marking complete.

## Delivery guardrails

- Read the approved specification, `GLOSSARY.md`, `AGENTS.md`, and relevant ADRs.
- Implement one vertical end-to-end behavior at a time using `/tdd`; no duplicate business rules in the UI or AI adapters.
- Respect Removed as reversible and Notes privacy at AI interfaces; never introduce permanent deletion.
- Use synthetic fixtures only; the public repository must never contain personal Task or Note data.
- This is an implementation ticket, not proof that implementation has begun.

## Comments

**2026-10-26 — implementation (AI implementer, awaiting independent review).** New crate `crates/tasknotes-cli` builds the `tasknotes` binary over the shared core; core schema v10 adds append-only `audit_entries`. Decisions (owner may revisit), details in the plan:
- Saya pilih crate terpisah `tasknotes-cli` (binary `tasknotes`) karena desktop app tidak perlu ikut ter-build dan Ticket 12 bisa memakai library kontrak JSON-nya.
- Saya pilih lokasi database sama dengan desktop release (`~/Library/Application Support/store.adamaja.tasknotes/tasknotes.db`), bisa diganti `--data-dir` atau `TASKNOTES_DATA_DIR`, karena CLI harus bekerja pada data yang sama tanpa setelan tambahan.
- Saya pilih izin lewat file `ai-access.json` di folder data yang hanya bisa dibuat `tasknotes access grant` dari terminal interaktif (orangnya mengetik `allow`), karena klien AI biasanya tidak punya terminal sehingga tidak bisa memberi izin pada dirinya sendiri. Tanpa izin, semua perintah data ditolak `not_trusted` (exit 6) sebelum database dibuka atau dibuat. Ini batas izin antarmuka, bukan perlindungan dari proses lokal yang mengubah file langsung (Q47, ADR-0004).
- Saya pilih Audit Entry ditulis core dalam transaksi yang sama dengan perubahannya, untuk semua perubahan Task dan List dari desktop maupun CLI; isinya hanya id, aksi, antarmuka, hasil, kode error dan revisi, tanpa teks pengguna. Penolakan yang dicatat: `revision_conflict` dan `removed`. Kesalahan input tidak dicatat. Perubahan Note/folder/link menyusul di Ticket 11 bersama aturan privasinya.
- Saya pilih kode error stabil di core (`TaskError::code()`) supaya CLI dan MCP sama; exit code 0/1/2/3/4/5/6/7 seperti di plan. Semua keluaran JSON di stdout, stderr kosong.

Evidence (implementer's own runs, 2026-10-26): `crates/tasknotes-core` cargo test 115 passed (new `tests/audit.rs`: 6, including v9→v10 migration and its rollback); `crates/tasknotes-cli` cargo test 20 passed (separate-process tests: access 5, tasks 6, errors 6, race 3 — the race runs 30 staggered desktop-versus-CLI rounds on one revision, every round exactly one write lands and the other gets `revision_conflict`; both sides won rounds); `src-tauri` cargo test 1 passed; cargo clippy `--all-targets -D warnings` and `cargo fmt --check` clean in all three; `pnpm test` 159 passed; `pnpm build` ok. The CLI's only dependencies are `serde`, `serde_json` and the core, so it opens no network port.

Open: desktop Settings has no access toggle yet (grant is CLI-terminal only); `/code-review` not run yet.

