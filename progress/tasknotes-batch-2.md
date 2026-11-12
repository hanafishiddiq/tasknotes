# Tasknotes batch 2

Tujuan akhir: Tasknotes di /Applications berisi 12 isi batch ini (PR #23, PR #24, Cmd+Enter, toast Task baru, Difficulty, Won't do, drag sort + Custom, nested List, sidebar baru + ikon/warna, Pomodoro + riwayatnya, klik lingkaran Task, hasil cek mute), dipasang dengan satu build. Cache build dihapus setelah build batch 3 (keputusan Han Q1, 2026-11-11; lihat progress/tasknotes-batch-3.md).

## Progres

- [x] Keputusan produk disepakati lewat grilling (2026-11-09, Q1–Q53 sampai 2026-11-11)
- [x] Keputusan dicatat di glosarium, ADR, dan spec
- [x] Tiket dipecah di .scratch/tasknotes-next/
- [x] PR #23 dan PR #24 di-review lalu di-merge ke main (bersama #22 ikon dan #25 spec, 2026-11-09)
- [x] Fitur kecil selesai: Cmd+Enter, toast Task baru, cek mute
  - [x] Cmd+Enter (PR #26) dan toast Task baru (PR #27) di main
  - [x] Cek CPU saat mute (Ticket 08), diukur di app baru (2026-11-12, AI lewat Cua atas izin Han)
    - Hasil: hanya hujan dimute ≈ 2 % CPU satu core (audio WebKit tetap jalan), keduanya dimute ≈ 0,2 %, suara nyala ≈ 1,7 %; usulan perbaikan menunggu keputusan Han (Ticket 08)
- [x] Fitur sedang selesai: sidebar (#28), Difficulty (#29), Won't Do (#30), ikon dan warna (#31)
- [x] Fitur besar selesai: drag sort + Custom (#33), nested List (#32), Pomodoro (#34), riwayat Pomodoro (#36)
- [x] Klik lingkaran Task untuk menyelesaikan (#35, Ticket 16)
- [x] Semua fitur di-merge ke main dan lolos cek gabungan (main 1bf9fe3, CI hijau)
- [x] Satu build dipasang ke /Applications (2026-11-11, setelah Han konfirmasi; database 15 → 23, 143 Task / 4 Note / 8 List utuh)
- [x] Han mengecek app asli (daftar 10 langkah)
  - Dicek AI lewat Cua atas izin Han (2026-11-12): 10/10 sesuai; data uji dipindah ke Removed
  - Temuan kecil: teks pembaca layar "1 minutes of focus today" di halaman Pomodoro (dicatat di batch 3 Ticket 04, belum diperbaiki)

## Perlu Han lakukan

- [x] Ubah pengaturan suara Tasknotes untuk pengukuran CPU saat mute (dikerjakan AI atas izin Han, pengaturan dikembalikan)
  - Cara: nyalakan hujan + suara klik lalu balas "1"; mute hujan saja lalu "2"; mute keduanya lalu "3"; layar tetap menyala, jangan klik Tasknotes selama 60 detik tiap langkah
  - Menahan: Cek CPU saat mute (Ticket 08), diukur di app baru
- [x] Cek app asli dengan daftar 10 langkah (toast, lingkaran, ⌘↵, Won't Do, Difficulty, drag, sidebar, ikon + sub-List, Pomodoro + notifikasi, halaman Pomodoro) (dikerjakan AI atas izin Han)
  - Cara: coba tiap langkah di Tasknotes, kabari nomor yang tidak sesuai
  - Menahan: Han mengecek app asli (daftar 10 langkah)
- [ ] Putuskan perbaikan audio saat hanya hujan dimute
  - Cara: pilih "matikan audio saat tidak ada suara" (klik pertama setelah hening bisa telat beberapa milidetik) atau "biarkan" (≈ 2 % CPU satu core selama app terbuka); detail di Ticket 08
  - Menahan: tidak ada; batch 2 sudah lengkap, perbaikan ini jadi tiket baru bila dipilih

## Riwayat sesi

- 2026-11-09 · 17451773
- 2026-11-12 · 17451773
