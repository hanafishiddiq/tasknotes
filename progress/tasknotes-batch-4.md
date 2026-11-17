# Tasknotes batch 4

Tujuan akhir: Tasknotes di /Applications berisi 12 perubahan batch 4 dari List "Tasknotes Features", dipasang dengan satu build dan dicek Han.

Keputusan Han (2026-11-17): 10 item kecil, ditambah 2 fitur baru (Task di beberapa List, kalimat pertama Note jadi judul); versi website, versi Windows, database online per akun, dan assign Task ditunda ke batch sendiri. Status In Progress ↔ Ready boleh diubah bebas (ADR 0006 diperbarui). Tanda kuning Task baru tetap terlihat 20 detik. Urutan: bug visual dulu.

Keputusan Han (2026-11-18), Han minta lanjut terus sampai selesai satu per satu: Pomodoro tetap jalan saat notifikasi mati, hanya diingatkan. Pindah ke Ready mempertahankan angka progress. Subtask berupa checklist sederhana dengan batas karakter sangat panjang; baris Task menampilkan 2/5, persen tetap manual. Gambar di catatan Task terletak di sela teks (catatan Task jadi editor seperti Notes), tanpa pratinjau di baris yang tertutup. Task di banyak List: tombol Remove menawarkan "hapus dari sini saja" atau "hapus dari semua". Judul Note = baris pertama.

## Progres

- [x] Fitur dibaca dan cakupan batch 4 disepakati (2026-11-17)
- [x] Nama panjang tidak menimpa apa pun di mana pun (nama folder di Notes dan semua tempat lain)
  - [x] Bug dari screenshot Han dibuat ulang di halaman uji (judul folder menimpa kolom pencarian)
  - [x] Semua halaman diperiksa dengan pendeteksi tumpang tindih, di lebar 1120 dan 760 px
  - [x] Diperbaiki dengan TDD: judul kolom Notes dan judul List satu baris berakhiran "…", nama lengkap muncul saat kursor diarahkan; tombol New note tidak ikut menyempit; semua dropdown berakhiran "…" (dibuktikan di WebKit macOS)
  - [x] Review dua sumbu; temuan diperbaiki (nama sintetis di halaman uji, kata panjang tanpa spasi di bar Undo dan teks lain, sidebar di jendela sempit)
  - [x] Draft PR dibuka dan CI hijau
- [x] Scrollbar tidak menutupi ikon kolom paling kanan di pemilih ikon List
  - [x] Penyebab dibuktikan di WebKit macOS: scrollbar melayang di atas grid, ikon terakhir sampai 5px dari tepi
  - [x] Diperbaiki dengan TDD: grid memberi ruang 12px di kanan; ikon kini 17px dari tepi, tetap 8 kolom
  - [x] Draft PR dibuka dan CI hijau
- [x] Kursor kuning saat mengetik di Notes lebih redup
  - [x] Han: "digelapkan sedikit saja" (2026-11-17); tema gelap #ffd60a → #d2b10e, tema terang tetap
  - [x] Diperbaiki dengan TDD (token --caret), seluruh tes lulus
  - [x] Draft PR dibuka dan CI hijau
- [x] Ikon Pomodoro di kiri bawah diganti
  - [x] Han memilih Timer (stopwatch), sama dengan ikon blok Pomodoro (2026-11-17)
  - [x] Diganti dengan TDD, seluruh tes lulus
  - [x] Draft PR dibuka dan CI hijau
- [x] Tanda kuning Task baru tetap terlihat 20 detik
  - [x] Han: berlaku untuk Task buatan sendiri dan dari AI; membuka Task tetap menghapus lebih cepat (2026-11-17)
  - [x] Dikerjakan dengan TDD: 20 detik sejak terlihat, lalu memudar 0,8 detik; DESIGN.md diperbarui
  - [x] Review: dua risiko kecil (dua aksi di saat yang sama) dan garis pemisah saat memudar diperbaiki; keputusan dicatat di issue batch 3
  - [x] Draft PR dibuka dan CI hijau
- [x] Tombol Hide/Show Completed
  - [x] Han: baris "Show Completed (N)" di bawah daftar, tersembunyi bawaan, diingat per List, pencarian dan filter Completed selalu menampilkan (2026-11-17)
  - [x] Inti (Rust): query bisa menyembunyikan Completed dan menghitungnya dengan filter yang sama
  - [x] Tampilan dengan TDD; dicek di halaman uji (tampil, sembunyi, angka benar, tanpa tumpang tindih)
  - [x] Review dua sumbu; diperbaiki: filter Ready/In Progress, Task yang sedang diedit tidak hilang, angka List lain tidak terbawa, kode dirapikan
  - [x] Draft PR dibuka dan CI hijau
- [x] Pomodoro meminta izin notifikasi sebelum bisa dipakai
  - [x] Han: Pomodoro tetap jalan, hanya diingatkan (2026-11-18)
  - [x] Dikerjakan dengan TDD: pengingat "Notifications are off" di sidebar dan halaman Pomodoro, tombol Turn on notifications (macOS bertanya, atau System Settings terbuka), dibaca ulang saat jendela kembali ke depan
  - [x] Review: format kode, tombol tidak bisa diklik berulang saat macOS masih bertanya (menunggu di thread terpisah), komentar tes; diperbaiki
  - [x] Draft PR #58, CI hijau
- [ ] Status In Progress ↔ Ready bisa diubah bebas
  - [x] Han: pindah ke Ready mempertahankan angka progress (2026-11-18)
  - [x] Aturan inti diubah dengan TDD (berlaku sama di app, CLI, MCP, perubahan massal); Ready tidak pernah 100%; tidak ada yang pindah ke Ready dengan sendirinya
  - [x] Tampilan: pilihan Ready tidak lagi diredupkan; baris Task Ready yang punya progress menampilkan persennya
  - [x] ADR 0006, GLOSSARY, AGENTS.md dan DECISIONS diperbarui
  - [x] Review: kode benar di semua jalur; dokumen yang masih memuat aturan lama (DESIGN.md, ADR 0006, ADR 0008) diperbarui
  - [ ] Draft PR #59, CI hijau
- [ ] Subtask
- [ ] Notes milik Task (catatan di dalam Task) bisa menyimpan gambar
- [ ] Satu Task bisa muncul di beberapa List sekaligus, identik (selesai di satu = selesai di semua)
- [ ] Kalimat pertama Note otomatis jadi judul bergaya heading (tebal dan lebih besar)
  - [x] Han: baris pertama, seperti Apple Notes (2026-11-18)
  - [x] Dikerjakan dengan TDD: baris pertama yang berisi teks tampil 24px tebal; isi Markdown tidak berubah; daftar, kutipan, kode di baris pertama tetap gayanya
  - [x] Dicek di halaman uji: judul 24px tebal, isi di bawahnya biasa
  - [x] Review: baris kedua di paragraf yang sama ikut jadi judul; diperbaiki (hanya teks sebelum Enter pertama)
- [ ] Semua tiket di main dan satu build dipasang ke /Applications
- [ ] App asli batch 4 dicek Han

## Perlu Han lakukan

## Riwayat sesi

- 2026-11-17 · a72eed00
