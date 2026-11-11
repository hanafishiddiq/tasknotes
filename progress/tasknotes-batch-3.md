# Tasknotes batch 3

Tujuan akhir: Tasknotes di /Applications berisi 5 perubahan batch 3 (tombol Remove merah, tanda kuning Task baru, menu klik kanan Task, halaman Pomodoro lengkap, angka Task belum selesai di sidebar), dipasang dengan satu build dan dicek Han, lalu cache build dan worktree yang sudah selesai dihapus.

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
- [x] Menu klik kanan Task (tiket 03) di main (PR #40, merge d9f0c22, CI 14/14 hijau)
  - [x] Tanya jawab Q20–Q24 (Q21: menu custom sesuai desain app)
  - [x] Dikerjakan dengan TDD (15 tes menu)
  - [x] Draft PR #40, review dua sumbu, temuan diperbaiki
  - [x] Di-merge setelah CI hijau (Q24)
- [x] Halaman Pomodoro lengkap (tiket 04) di main
  - [x] Tanya jawab Q25–Q30 (Q28: dropdown Task bergaya menu klik kanan; Q29: Pause/Resume)
  - [x] Han menyetujui pratinjau halaman (Q25): susunan A
    - [x] Pratinjau 1 dan 2 ditolak: terlalu ramai, Han minta gaya contoh dua panel
    - [x] Pratinjau 3: tiga susunan A/B/C, semua keadaan, tema gelap dan terang
    - [x] Han memilih susunan A (Q39)
    - [x] Start break saat fokus berjalan, menitnya tetap dihitung (Q43)
    - [x] Q40–Q42 dan Q44 dijawab "ya semua" (menit saja, bukan sesi)
  - [x] Inti: Pause/Resume, Start break lebih awal, total sepanjang waktu (commit 9ab3463, dc6a1f3)
  - [x] Shell: perintah baru, menu bar, notifikasi (commit 3280e46)
  - [x] Halaman A: timer, pilihan Task, durasi, garis hari, riwayat (commit 8133fc4; 1534 tes window hijau)
  - [x] Dicek di app debug dengan data contoh (tombol Pause tetap kuning, sudah diperbaiki)
  - [x] Draft PR, review dua sumbu, merge setelah Han konfirmasi
    - [x] Draft PR #41 dibuka, review dua sumbu selesai
    - [x] Temuan review diperbaiki (cincin jeda redup lagi, garis hari, pemilih Task semua halaman, satu aturan hitung)
    - [x] Han memutuskan tombol Resume di blok sidebar: dipertahankan (Q47)
    - [x] CI hijau (14 cek), di-merge setelah Han konfirmasi (PR #41, commit 27f6749)
- [ ] Angka Task belum selesai di sidebar (tiket 05) di main
  - [x] Diminta Han dan dijawab (Q45 Ready + In Progress; Q46 semua folder, bisa diatur di Settings)
  - [ ] Bentuk pengaturan di Settings ditanyakan saat tiket dimulai
- [ ] Semua tiket di main dan lolos CI gabungan
- [ ] Satu build dipasang ke /Applications (setelah Han konfirmasi)
- [ ] Han mengecek app asli batch 3
- [ ] Cache build dan worktree yang sudah selesai dihapus (batch 2 dan 3)

## Perlu Han lakukan



## Riwayat sesi

- 2026-11-11 · 17451773
