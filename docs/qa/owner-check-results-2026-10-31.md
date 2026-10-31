# Hasil cek klik oleh AI, 2026-10-31

AI menjalankan langkah [cek klik](owner-check-2026-10-30.md) yang bisa diotomatiskan pada Tasknotes QA dengan data sintetis. Data asli tidak tersentuh. Hasil ini mengurangi kerja pemilik, tetapi tidak menggantikan persetujuannya.

Cara uji: klik dan ketik sungguhan lewat accessibility (Cua Driver), screenshot, CLI, dan pembacaan database QA secara read-only.

## Hasil per langkah

| Langkah | Hasil | Catatan |
| --- | --- | --- |
| 1–3 | Lolos | Task bertahan setelah aplikasi ditutup. Light bertahan setelah dibuka ulang; Match system mengikuti macOS yang sedang gelap. |
| 5–9 | Lolos | Urutan grup benar. Riwayat progres tampil terbaru di atas. "Unsaved edits" muncul dan teksnya tetap ada. Tidak ada tombol Delete, Erase, atau Empty. |
| 10–13 | Lolos | Note tersimpan saat aplikasi ditutup tepat setelah mengetik. Batas 5 hasil Notes di pencarian tidak diuji di sini karena hanya ada 1 Note. |
| 15–16 | Lolos | Saat palette terbuka, ⌘N, ⌘F, dan ⌘, tidak bereaksi. ⌘Z mengembalikan Task di luar kolom teks dan hanya membatalkan ketikan di dalamnya. |
| 19 | Lolos, menurut screenshot | Tema terang dan gelap terbaca, tetapi rasa keterbacaan tetap penilaian pemilik. |
| 20 | Sebagian | Aplikasi tetap jalan setelah jendela ditutup dan ikon ada. Menu pada ikon tidak bisa dibuka oleh AI. |
| 21–22 | Lolos | Shortcut global bekerja dengan ⌃⌥T saat jendela tertutup. ⌘N ditolak dengan penjelasan. |
| 24 | Sebagian | CPU rata-rata 0,9% saat diam, tetapi AI tidak bisa memastikan hujan sedang berbunyi. |
| 25 | Lolos | Backup harian ada, backup manual jalan, restore mengembalikan data, dan label "Before a restore" muncul. |
| 26 | Lolos | Note privat hanya ikut ekspor kalau opsinya dicentang. |
| 27 | Lolos | Note privat tidak muncul di `note list`, `note get`, `audit list`, dan `task notes`. `access grant` menolak AI dan hanya menerima konfirmasi orang di terminal. |
| 28 | Lolos | Approve memindahkan dua Task ke Removed, dan audit mencatat CLI sebagai peminta. Decline tidak mengubah apa pun. |
| 29 | Lolos | Setelah restore, Note tetap privat, keputusan tetap declined, dan Waiting for approval tidak muncul lagi. |

## Temuan

| Temuan | Status |
| --- | --- |
| Jendela membeku selama dialog izin Downloads dari macOS menunggu jawaban. | Diperbaiki: ekspor berjalan di thread latar. Bukti debugger: `linkat` dipanggil dari `tokio-rt-worker`. Dialog tidak muncul lagi, jadi skenario "dialog terbuka" belum terulang. |
| Nama file ekspor memakai UTC, sehingga berbeda dari jam Mac. | Diperbaiki dan dicek di aplikasi. |
| Nama folder terpotong di sidebar walaupun ruangnya ada. | Diperbaiki dan dicek di aplikasi. |
| ⌃⌥Space tidak terekam: macOS memakainya untuk ganti sumber input di Mac pemilik. | Panduan diganti ke ⌃⌥T. Kekurangan yang tersisa: aplikasi tidak bisa memberi tahu, karena tombolnya tidak pernah sampai ke aplikasi. |
| Task tanpa deadline tampak bertanggal hari ini, padahal kosong. | Diperbaiki atas pilihan pemilik: kolomnya menulis "No deadline" dan menampilkan tanggal saat difokus. Dicek di aplikasi pada kedua tema. |
| "Save changes" menutup panel Task, sedangkan "Save tags and deadline" tidak. | Belum ditemukan keputusan tertulis; menunggu penilaian pemilik. |
| Sorotan sidebar tampak tertinggal di All tasks pada screenshot. | Bukan bug aplikasi: jendela yang tertutup jendela lain berhenti digambar ulang. Saat jendelanya terlihat, sorotannya pindah dengan benar. |
| Dua backup dalam menit yang sama punya nama aksesibilitas yang sama ("Restore the backup from … 12:46"). | Belum diperbaiki; kecil. |

## Sisa untuk pemilik

Sekitar 15 menit (perkiraan), kecuali waktu pembuatan data besar:

1. Langkah 4: matikan Wi-Fi, ulangi langkah 2.
2. Langkah 14: nyalakan Keyboard navigation, lalu uji Tab, ↑/↓, Return, Esc, dan cincin fokus.
3. Langkah 17: jelajahi aplikasi dengan VoiceOver.
4. Langkah 18: nyalakan Reduce motion.
5. Langkah 20: buka menu dari ikon menu bar, lalu pilih Open Tasknotes.
6. Langkah 23: dengarkan suara hujan dan suara klik. Saat hujan berbunyi, sekalian lihat CPU di Activity Monitor (langkah 24).
7. Langkah 30–32: rasa scroll, pencarian, dan mengetik di data besar (`scripts/qa/run-owner-check.sh --large`, sekitar 7 menit).
