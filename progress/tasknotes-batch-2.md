# Tasknotes batch 2

Tujuan akhir: Tasknotes di /Applications berisi 12 isi batch ini (PR #23, PR #24, Cmd+Enter, toast Task baru, Difficulty, Won't do, drag sort + Custom, nested List, sidebar baru + ikon/warna, Pomodoro + riwayatnya, klik lingkaran Task, hasil cek mute), dipasang dengan satu build, lalu cache build dihapus.

## Progres

- [x] Keputusan produk disepakati lewat grilling (2026-11-09, Q1–Q53 sampai 2026-11-11)
- [x] Keputusan dicatat di glosarium, ADR, dan spec
- [x] Tiket dipecah di .scratch/tasknotes-next/
- [x] PR #23 dan PR #24 di-review lalu di-merge ke main (bersama #22 ikon dan #25 spec, 2026-11-09)
- [ ] Fitur kecil selesai: Cmd+Enter, toast Task baru, cek mute
  - [x] Cmd+Enter (PR #26) dan toast Task baru (PR #27) di main
  - [ ] Cek CPU saat mute (Ticket 08), diukur di app baru
- [x] Fitur sedang selesai: sidebar (#28), Difficulty (#29), Won't Do (#30), ikon dan warna (#31)
- [x] Fitur besar selesai: drag sort + Custom (#33), nested List (#32), Pomodoro (#34), riwayat Pomodoro (#36)
- [x] Klik lingkaran Task untuk menyelesaikan (#35, Ticket 16)
- [x] Semua fitur di-merge ke main dan lolos cek gabungan (main 1bf9fe3, CI hijau)
- [x] Satu build dipasang ke /Applications (2026-11-11, setelah Han konfirmasi; database 15 → 23, 143 Task / 4 Note / 8 List utuh)
- [ ] Han mengecek app asli (daftar 10 langkah)
- [ ] Cache build dan worktree yang sudah selesai dihapus (Han: tunggu cek app selesai)

## Perlu Han lakukan

- [ ] Ubah pengaturan suara Tasknotes untuk pengukuran CPU saat mute
  - Cara: nyalakan hujan + suara klik lalu balas "1"; mute hujan saja lalu "2"; mute keduanya lalu "3"; layar tetap menyala, jangan klik Tasknotes selama 60 detik tiap langkah
  - Menahan: Cek CPU saat mute (Ticket 08), diukur di app baru
- [ ] Cek app asli dengan daftar 10 langkah (toast, lingkaran, ⌘↵, Won't Do, Difficulty, drag, sidebar, ikon + sub-List, Pomodoro + notifikasi, halaman Pomodoro)
  - Cara: coba tiap langkah di Tasknotes, kabari nomor yang tidak sesuai
  - Menahan: Han mengecek app asli (daftar 10 langkah)

## Riwayat sesi

- 2026-11-09 · 17451773
