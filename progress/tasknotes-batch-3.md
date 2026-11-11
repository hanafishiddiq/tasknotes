# Tasknotes batch 3

Tujuan akhir: Tasknotes di /Applications berisi 4 perubahan batch 3 (tombol Remove merah, tanda kuning Task baru, menu klik kanan Task, halaman Pomodoro lengkap), dipasang dengan satu build dan dicek Han, lalu cache build dan worktree yang sudah selesai dihapus.

## Progres

- [x] Fitur baru dibaca dan urutan kerja disepakati (Q1–Q3, 2026-11-11)
- [x] Tombol Remove merah (tiket 01) di main (PR #38, merge eae7390, CI 14/14 hijau)
  - [x] Dikerjakan dengan TDD
  - [x] Draft PR #38, review dua sumbu, temuan diperbaiki
  - [x] Di-merge setelah Han konfirmasi (Q9)
- [x] Tanda kuning Task baru (tiket 02) di main (PR #39, merge 83aa171, CI 14/14 hijau)
  - [x] Tanya jawab, termasuk konflik dengan keputusan 9 Nov
    - [x] Q5–Q8 dijawab (Q8: Task dari AI juga diberi tanda)
    - [x] Q4, Q10, Q13 dijawab (toast tetap, 10 detik + tombol X; AI lewat CLI/MCP; tanpa toast untuk AI)
    - [x] Klarifikasi Q14–Q16 (10 detik sejak terlihat; tidak disimpan; Task sendiri ikut Q6)
  - [x] Bagian 1: toast 10 detik + tombol X, tanpa kilatan (commit fb65e02, tes lokal hijau)
  - [x] Bagian 2: tanda kuning Task sendiri dan Task dari AI (kuning 50% demi kontras)
  - [x] Draft PR #39, review dua sumbu, temuan diperbaiki (commit 154824d)
  - [x] Jendela tersembunyi lewat menu bar: diputuskan Han (Q17: tetap diberi tanda, tanpa ubah kode)
  - [x] Di-merge setelah CI hijau dan Han konfirmasi (Q19)
- [ ] Menu klik kanan Task (tiket 03) di main
  - [x] Tanya jawab Q20–Q24 (Q21: menu custom sesuai desain app)
  - [x] Dikerjakan dengan TDD (15 tes menu)
  - [x] Draft PR #40, review dua sumbu, temuan diperbaiki
  - [ ] Di-merge setelah CI hijau (Q24)
- [ ] Halaman Pomodoro lengkap (tiket 04) di main
- [ ] Semua tiket di main dan lolos CI gabungan
- [ ] Satu build dipasang ke /Applications (setelah Han konfirmasi)
- [ ] Han mengecek app asli batch 3
- [ ] Cache build dan worktree yang sudah selesai dihapus (batch 2 dan 3)

## Perlu Han lakukan


## Riwayat sesi

- 2026-11-11 · 17451773
