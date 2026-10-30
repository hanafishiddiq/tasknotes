# Cek klik sebelum merge

Daftar ini menggabungkan checklist semua tiket jadi alur yang paling penting. Perkiraan waktu sekitar 45 menit untuk bagian A–G. Semuanya memakai aplikasi **Tasknotes QA** dengan folder data sendiri, jadi data asli tidak tersentuh.

**Mulai:** dari folder proyek, jalankan:

```bash
scripts/qa/run-owner-check.sh --fresh
```

Catat yang terasa salah atau aneh, cukup nomornya dan apa yang terjadi. Jangan perbaiki apa pun sendiri.

## A. Pertama kali buka

1. Aplikasi terbuka di All tasks, tanpa contoh Task. Sidebar menampilkan All tasks, Inbox, Lists, All notes, Removed, dan Settings.
2. Tulis judul Task, tekan Return. Task muncul. Tutup dengan ⌘Q, buka lagi: masih ada.
3. Settings → Appearance: coba Light, Dark, Match system. Pilihannya bertahan setelah aplikasi dibuka ulang.
4. Matikan Wi-Fi, ulangi nomor 2: semua tetap jalan, tanpa pesan error.

## B. Task

5. Buat dua List. Pindahkan Task ke List, beri tag dan deadline, ubah status dan prioritas. Urutan grup tetap In Progress → Ready → Completed.
6. Buka Task, isi progres, Next Step, dan catatan. Riwayat progresnya tampil urut.
7. Buka Task, ketik di judul tanpa menyimpan, gulir jauh lalu kembali: teksnya masih ada dan posisi gulir tidak melompat.
8. Pindahkan Task ke Removed, lalu Undo. Pindahkan lagi, buka Removed, lalu Restore. Rincian dan riwayatnya kembali utuh.
9. Di mana pun, tidak ada tombol Delete, Erase, atau Empty.

## C. Notes

10. Buat folder dan subfolder, buat Note, ketik lalu tunggu sebentar. Tutup dengan ⌘Q tepat setelah mengetik: teksnya tersimpan.
11. Pin sebuah Note, pindahkan ke folder lain, lalu cari satu kata dari isinya. Kata itu tersorot di hasil pencarian.
12. Hubungkan Note ke Task dari kedua sisi. Lepaskan dari sisi Note, lalu Undo. Task dan Note sendiri tidak berubah.
13. Di pencarian All tasks, ketik kata yang sama: bagian Notes muncul di atas Task, paling banyak 5 hasil.

## D. Keyboard, tampilan, dan aksesibilitas

14. Hanya dengan keyboard: Tab ke baris Task, ↑/↓ berpindah, Return membuka, Esc menutup. Setiap kontrol yang terfokus menampilkan cincin fokus kuning.
15. ⌘K membuka palette, Esc menutup. Coba ⌘N, ⌘F, dan ⌘,. Saat palette terbuka, ketiganya tidak bereaksi di belakangnya.
16. Hapus Task lalu tekan ⌘Z saat fokus di luar kolom teks: Task kembali. Di dalam kolom teks, ⌘Z hanya membatalkan ketikan.
17. Nyalakan VoiceOver (⌘F5), jelajahi sidebar, baris Task, dan Settings. Setiap kontrol punya nama yang masuk akal.
18. System Settings → Accessibility → Display → Reduce motion ON: animasi berhenti dan semua tetap jalan.
19. Di tema gelap dan terang, teks tetap mudah dibaca, termasuk tag, kata yang tersorot, dan Note yang terpilih.

## E. Menu bar, Quick Capture, dan suara

20. Settings → Menu bar and Quick Capture: nyalakan menu bar. Tutup jendela: aplikasi tetap jalan dan ikon ada di menu bar. Buka lagi dari ikon itu.
21. Nyalakan shortcut global, pindah ke aplikasi lain, tekan ⌃⌥Space: Tasknotes muncul dengan kolom New task terfokus. Isi dan Return.
22. Coba pasang ⌘N sebagai shortcut: ditolak dengan penjelasan.
23. Settings → Sound: hujan terdengar pelan, tidak mengganggu, dan berulang tanpa jeda atau klik. Atur volumenya, lalu mute. Suara klik juga pelan.
24. Di Activity Monitor, CPU Tasknotes QA saat hujan menyala dan aplikasi diam tetap rendah. Catat angkanya.

## F. Backup, ekspor, dan AI

25. Settings → Backups: backup harian sudah ada. Buat backup manual. Ubah sebuah Task, lalu Restore backup itu: datanya kembali, dan ada backup "Pre-restore".
26. Ekspor ke JSON dan Markdown. Buka hasilnya. Coba dengan dan tanpa "Include Notes marked Exclude from AI".
27. Tandai sebuah Note Exclude from AI. Dengan perintah CLI yang ditampilkan skrip di atas, jalankan `access grant`, lalu `note list`: Note itu tidak muncul. Riwayatnya juga tidak muncul di `audit list`.
28. Lewat CLI, jalankan `bulk preview --action remove_tasks --id … --id …` untuk dua Task. Di aplikasi, sidebar menampilkan **Waiting for approval**. Buka, lalu Approve: kedua Task masuk Removed. Ulangi, lalu Decline: tidak ada yang berubah.
29. Ulangi nomor 25 dengan Note dari nomor 27 yang privat setelah backup dibuat: Note tetap privat setelah restore. Permintaan massal yang sudah Han putuskan tidak muncul menunggu lagi.

## G. Rasa di data besar

Jalankan `scripts/qa/run-owner-check.sh --large`, yang membuat 50.000 Task dan 10.000 Note sintetis (sekitar 5 menit).

30. Gulir daftar Task dengan trackpad, cepat dan pelan, dengan baris tertutup lalu satu baris terbuka: terasa mulus, tanpa patah atau baris kosong. Ini satu-satunya hal yang tidak bisa diukur otomatis, sekaligus pemeriksaan keluhan lag lama (Issue 19).
31. Ketik di kolom pencarian: hasil muncul tanpa jeda mengganggu, termasuk saat baru satu huruf. Ulangi di Notes.
32. Buka beberapa Note panjang dan ketik: tidak tersendat.

## H. Hanya kalau ada waktu

Langkah rinci per fitur ada di tiket: [07](../../.scratch/tasknotes-v1/issues/07-note-organization-and-search.md), [08](../../.scratch/tasknotes-v1/issues/08-reversible-task-note-links.md), [09](../../.scratch/tasknotes-v1/issues/09-removed-and-restore.md), [12](../../.scratch/tasknotes-v1/issues/12-mcp-stdio-and-safe-bulk-mutations.md), [14](../../.scratch/tasknotes-v1/issues/14-desktop-interaction-accessibility-polish.md), dan [15](../../.scratch/tasknotes-v1/issues/15-menu-bar-global-quick-capture.md).
