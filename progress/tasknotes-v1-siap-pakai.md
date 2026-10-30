# Tasknotes v1 siap pakai

Tujuan akhir: Tasknotes terpasang di Mac Han dan dipakai dengan data asli, setelah Task, Notes, CLI, MCP, backup, menu bar, dan suara lolos uji akhir di aplikasi yang sudah dipaketkan.

## Progres

- [x] Push tiket 12 dan 13, selesaikan bentroknya, lalu jalankan semua tes gabungan
  - [x] Branch tiket 12 dan 13 di-push
  - [x] Bentrok 12 file selesai, semua penulisan lewat pagar restore
  - [x] Server MCP mengambil backup harian
  - [x] Review independen selesai, 4 temuan teknis diperbaiki
  - [x] Hasil gabungan di-push dan CI hijau
- [x] Terapkan keputusan Han soal dua perilaku restore
- [x] Rapikan status tiket dan deskripsi PR, lalu selesaikan review yang masih terbuka
  - [x] Tambah tes CLI dan MCP ke CI
  - [x] Temuan re-review tiket 07, 11, dan 14 diperbaiki
- [ ] Kerjakan tiket 17: ukur performa dengan 50.000 Task dan 10.000 Note sintetis
  - [x] Dataset sintetis dan alat ukur jadi
  - [x] Pencarian dan paging lewat core, CLI, dan MCP memenuhi target
  - [x] Pencarian Note satu huruf diperbaiki, dari p95 3 detik ke 90 ms
  - [ ] Ukur scroll dan pencarian di aplikasi dengan layar menyala
- [ ] Kerjakan tiket 18: paket aplikasi, uji alur UI/CLI/MCP, latihan restore, aksesibilitas, audit privasi repo
  - [ ] Bereskan kegagalan langka saat dua proses membuka database bersamaan
- [ ] Han mengecek aplikasi dengan data sintetis, lalu menyetujui merge ke main
- [ ] Backup data asli, lalu pasang di Mac Han

## Perlu Han lakukan

- [ ] Jalankan pengukuran di layar
  - Cara: buka kunci Mac, jalankan `/Users/hanafi/Desktop/Rumah/Playground/tasknotes/.worktrees/ticket-17/scripts/qa/run-ticket17-ui.sh` di Terminal, lalu biarkan jendelanya sekitar 3 menit dan bilang "sudah".
  - Menahan: Kerjakan tiket 17: ukur performa dengan 50.000 Task dan 10.000 Note sintetis

- [ ] Cek klik aplikasi dengan data sintetis
  - Cara: AI menggabungkan checklist semua tiket jadi satu daftar, lalu Han mengikutinya di aplikasi uji.
  - Menahan: Han mengecek aplikasi dengan data sintetis, lalu menyetujui merge ke main
- [ ] Persetujuan merge dan pemasangan ke data asli
  - Cara: bilang "merge" setelah cek klik beres; persetujuan pasang ke data asli diminta terpisah.
  - Menahan: Backup data asli, lalu pasang di Mac Han

## Riwayat sesi

- 2026-10-30 · af2e1d1e
