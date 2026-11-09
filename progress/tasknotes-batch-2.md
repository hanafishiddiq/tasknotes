# Tasknotes batch 2

Tujuan akhir: Tasknotes di /Applications berisi 11 isi batch ini (PR #23, PR #24, Cmd+Enter, toast Task baru, Difficulty, Won't do, drag sort + Custom, nested List, sidebar baru + ikon/warna, Pomodoro, hasil cek mute), dipasang dengan satu build, lalu cache build dihapus.

## Progres

- [x] Keputusan produk disepakati lewat grilling (2026-11-09)
- [x] Keputusan dicatat di glosarium, ADR, dan spec
- [x] Tiket dipecah di .scratch/tasknotes-next/
- [x] PR #23 dan PR #24 di-review lalu di-merge ke main (bersama #22 ikon dan #25 spec, 2026-11-09)
- [ ] Fitur kecil selesai: Cmd+Enter, toast Task baru, cek mute
  - [x] Cmd+Enter (PR #26) dan toast Task baru (PR #27) di main
  - [ ] Cek CPU saat mute (Ticket 08)
- [ ] Fitur sedang selesai: Difficulty, Won't do, sidebar + ikon/warna
  - [x] Sidebar (PR #28) dan Difficulty (PR #29) di main
  - [ ] Won't Do (Ticket 10)
  - [ ] Ikon dan warna (Ticket 01)
- [ ] Fitur besar selesai: drag sort + Custom, nested List, Pomodoro
- [ ] Semua fitur di-merge ke main dan lolos cek gabungan
- [ ] Satu build dipasang ke /Applications (setelah Han konfirmasi)
- [ ] Cache build dan worktree yang sudah selesai dihapus

## Perlu Han lakukan

- [ ] Ubah pengaturan suara Tasknotes untuk pengukuran CPU saat mute
  - Cara: nyalakan hujan + suara klik lalu balas "1"; mute hujan saja lalu "2"; mute keduanya lalu "3"; layar tetap menyala, jangan klik Tasknotes selama 60 detik tiap langkah
  - Menahan: Cek CPU saat mute (Ticket 08)

## Riwayat sesi

- 2026-11-09 · 17451773
