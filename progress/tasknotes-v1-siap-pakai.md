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
- [x] Kerjakan tiket 17: ukur performa dengan 50.000 Task dan 10.000 Note sintetis
  - [x] Dataset sintetis dan alat ukur jadi
  - [x] Pencarian dan paging lewat core, CLI, dan MCP memenuhi target
  - [x] Pencarian Note satu huruf diperbaiki, dari p95 3 detik ke 90 ms
  - [x] Ukur scroll dan pencarian di aplikasi dengan layar menyala
- [x] Kerjakan bagian otomatis tiket 18: paket aplikasi, uji alur UI/CLI/MCP, latihan restore, aksesibilitas, audit privasi repo
  - [x] Bereskan kegagalan langka saat dua proses membuka database bersamaan
  - [x] Uji alur UI, CLI, dan MCP serta uji mati mendadak lulus
  - [x] Latihan upgrade dari schema lama ke 15 tanpa data berubah
  - [x] Warna diperbaiki sesuai keputusan Han, tes aksesibilitas otomatis lulus
  - [x] Restore berulang lolos pemeriksa acak 2.000 urutan
  - [x] Paket aplikasi menyala dua kali di schema 15, tanpa koneksi jaringan
  - [x] Audit privasi repo dan daftar kesiapan rilis terbit
  - [x] CI hijau untuk hasil gabungan tiket 18
- [ ] Han mengecek aplikasi dengan data sintetis, lalu menyetujui merge ke main
- [ ] Backup data asli, lalu pasang di Mac Han

## Perlu Han lakukan

- [x] Jalankan pengukuran di layar
  - Cara: sudah dijalankan Han, hasilnya sah (`frame_timings_valid: true`).
  - Menahan: Kerjakan tiket 17: ukur performa dengan 50.000 Task dan 10.000 Note sintetis

- [ ] Cek klik aplikasi dengan data sintetis
  - Cara: di Terminal masuk ke `.worktrees/integration` di dalam proyek, jalankan `scripts/qa/run-owner-check.sh --fresh`, lalu ikuti `docs/qa/owner-check-2026-10-30.md`, sekitar 45 menit.
  - Menahan: Han mengecek aplikasi dengan data sintetis, lalu menyetujui merge ke main
- [ ] Persetujuan merge dan pemasangan ke data asli
  - Cara: bilang "merge" setelah cek klik beres; persetujuan pasang ke data asli diminta terpisah.
  - Menahan: Backup data asli, lalu pasang di Mac Han

## Riwayat sesi

- 2026-10-30 · af2e1d1e
