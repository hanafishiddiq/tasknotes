# Tasknotes batch 2

Tujuan akhir: Tasknotes di /Applications berisi 12 isi batch ini (PR #23, PR #24, Cmd+Enter, toast Task baru, Difficulty, Won't do, drag sort + Custom, nested List, sidebar baru + ikon/warna, Pomodoro + riwayatnya, klik lingkaran Task, hasil cek mute), dipasang dengan satu build; ditambah perbaikan audio saat hanya hujan dimute (Ticket 17, pilihan Han 2026-11-12). Cache build dihapus setelah build batch 3 (keputusan Han Q1, 2026-11-11; lihat progress/tasknotes-batch-3.md).

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
- [x] Perbaikan audio: perangkat audio istirahat di antara bunyi saat hujan tidak berbunyi (Ticket 17)
  - [x] Dibuat test-first, direview dua sumbu, Draft PR #48 (2026-11-12)
  - [x] Merge dan pasang ke /Applications setelah izin Han (2026-11-12; #48 fc94095, #47 a33441f; cadangan pre-ticket17-20261112 terverifikasi)
  - [x] Ukur ulang CPU: hanya hujan dimute dan app diam → audio WebKit 0,00 % / 0,03 % (sebelumnya 1,26 %), total Tasknotes 0,31 % / 0,76 % (sebelumnya ≈ 2 %)
  - [x] Han mendengar bunyi pertama setelah diam lebih dari 5 detik ("Terdengar", 2026-11-12)
- [ ] Sisa cek native setelah batch 2 dan 3 dirapikan (2026-11-12)
  - [x] Centang tiket yang sudah terbukti (batch 3 tiket 01–05, Ticket 05, Ticket 15)
  - [x] Teks pembaca layar "1 minute of focus today" diperbaiki (ikut build berikutnya)
  - [x] Ticket 05 di app asli: Task dari CLI muncul sendiri dalam 4–5 detik, ketikan yang belum disimpan tetap ada
  - [ ] Tampilan halaman Pomodoro di tema gelap, terang, dan jendela 400 px (Ticket 14)
  - [ ] Sisa notifikasi Pomodoro, butuh telinga dan klik Han (Ticket 02, batch 3 Issue 06)

## Perlu Han lakukan

- [x] Ubah pengaturan suara Tasknotes untuk pengukuran CPU saat mute (dikerjakan AI atas izin Han, pengaturan dikembalikan)
  - Cara: nyalakan hujan + suara klik lalu balas "1"; mute hujan saja lalu "2"; mute keduanya lalu "3"; layar tetap menyala, jangan klik Tasknotes selama 60 detik tiap langkah
  - Menahan: Cek CPU saat mute (Ticket 08), diukur di app baru
- [x] Cek app asli dengan daftar 10 langkah (toast, lingkaran, ⌘↵, Won't Do, Difficulty, drag, sidebar, ikon + sub-List, Pomodoro + notifikasi, halaman Pomodoro) (dikerjakan AI atas izin Han)
  - Cara: coba tiap langkah di Tasknotes, kabari nomor yang tidak sesuai
  - Menahan: Han mengecek app asli (daftar 10 langkah)
- [x] Putuskan perbaikan audio saat hanya hujan dimute (Han: "Ya, perbaiki", 2026-11-12 → Ticket 17)
- [x] Izinkan merge PR #48 dan pasang build baru ke /Applications (Han: "Ya, merge & pasang", 2026-11-12)
- [x] Dengarkan bunyi pertama setelah Tasknotes diam lebih dari 5 detik (Han: terdengar)
  - Cara: biarkan Tasknotes tanpa klik lebih dari 5 detik, lalu tambah atau selesaikan satu Task; bunyinya harus terdengar seperti biasa
  - Menahan: Han mendengar bunyi pertama setelah diam (Ticket 17)
- [ ] Cek sisa notifikasi Pomodoro (opsional, sekitar 5 menit)
  - Cara:
    1. Halaman Pomodoro › ikon durasi: Focus 1, Break 1, lalu Save. Pilih Task apa saja, Start focus, lalu pindah ke app lain.
    2. Saat "Focus done" muncul, klik tombol Count overtime di notifikasinya. Harapan: Break langsung mulai, sesi tercatat Finished dengan Overtime.
    3. Satu menit kemudian muncul "Break over" (tombol Start focus) dan menu bar menulis "☕ Done" (ikon ada di balik « di menu bar). Klik badan notifikasinya, bukan tombolnya. Harapan: jendela Tasknotes muncul.
    4. Nyalakan Silent di sheet durasi, lalu Start focus. "Focus done" harus muncul tanpa bunyi; jawab lewat menu 🍅 di menu bar (Start break). Harapan: notifikasinya hilang.
    5. Start focus lagi; saat "Focus done" tampil, keluar dari Tasknotes (⌘Q). Harapan: notifikasinya hilang dan sesi tersimpan.
    6. Kembalikan Focus 25, Break 5, Silent mati.
  - Menahan: Sisa notifikasi Pomodoro (Ticket 02, batch 3 Issue 06); bukan penghalang pemakaian

## Riwayat sesi

- 2026-11-09 · 17451773
- 2026-11-12 · 17451773
