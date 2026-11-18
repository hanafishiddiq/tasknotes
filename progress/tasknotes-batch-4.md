# Tasknotes batch 4

Tujuan akhir: Tasknotes di /Applications berisi 12 perubahan batch 4 dari List "Tasknotes Features", dipasang dengan satu build dan dicek Han.

Keputusan Han (2026-11-17): 10 item kecil, ditambah 2 fitur baru (Task di beberapa List, kalimat pertama Note jadi judul); versi website, versi Windows, database online per akun, dan assign Task ditunda ke batch sendiri. Status In Progress ↔ Ready boleh diubah bebas (ADR 0006 diperbarui). Tanda kuning Task baru tetap terlihat 20 detik. Urutan: bug visual dulu.

## Progres

- [x] Fitur dibaca dan cakupan batch 4 disepakati (2026-11-17)
- [ ] Nama panjang tidak menimpa apa pun di mana pun (nama folder di Notes dan semua tempat lain)
  - [x] Bug dari screenshot Han dibuat ulang di halaman uji (judul folder menimpa kolom pencarian)
  - [x] Semua halaman diperiksa dengan pendeteksi tumpang tindih, di lebar 1120 dan 760 px
  - [x] Diperbaiki dengan TDD: judul kolom Notes dan judul List satu baris berakhiran "…", nama lengkap muncul saat kursor diarahkan; tombol New note tidak ikut menyempit; semua dropdown berakhiran "…" (dibuktikan di WebKit macOS)
  - [x] Review dua sumbu; temuan diperbaiki (nama sintetis di halaman uji, kata panjang tanpa spasi di bar Undo dan teks lain, sidebar di jendela sempit)
  - [ ] Draft PR dibuka dan CI hijau
- [ ] Scrollbar tidak menutupi ikon kolom paling kanan di pemilih ikon List
- [ ] Kursor kuning saat mengetik di Notes lebih redup
- [ ] Ikon Pomodoro di kiri bawah diganti
- [ ] Tanda kuning Task baru tetap terlihat 20 detik
- [ ] Tombol Hide/Show Completed
- [ ] Pomodoro meminta izin notifikasi sebelum bisa dipakai
- [ ] Status In Progress ↔ Ready bisa diubah bebas
- [ ] Subtask
- [ ] Notes milik Task (catatan di dalam Task) bisa menyimpan gambar
- [ ] Satu Task bisa muncul di beberapa List sekaligus, identik (selesai di satu = selesai di semua)
- [ ] Kalimat pertama Note otomatis jadi judul bergaya heading (tebal dan lebih besar)
- [ ] Semua tiket di main dan satu build dipasang ke /Applications
- [ ] App asli batch 4 dicek Han

## Perlu Han lakukan

## Riwayat sesi

- 2026-11-17 · a72eed00
