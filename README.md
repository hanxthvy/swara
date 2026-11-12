# Swara

Pemutar musik desktop untuk YouTube Music, ditulis penuh dalam Rust. Tidak ada
Chromium, tidak ada webview, tidak ada server lokal: satu binary yang membuka
jendelanya sendiri dan memutar audionya sendiri.

Awalnya proyek ini berjalan sebagai backend Python + UI HTML yang dibuka lewat
Chromium mode `--app`. Swara adalah tulis ulang native-nya.

## Fitur

- Beranda dari feed YouTube Music (`FEmusic_home`): baris playlist, tangga lagu,
  "dengarkan lagi".
- Pencarian lagu lengkap dengan saran saat mengetik.
- Halaman album/playlist: daftar trek, tombol putar semua.
- Pemutar: seek, volume, transport, antrean otomatis dari mix `RDAMVM`.
- Warna antarmuka diturunkan dari sampul lagu yang sedang diputar.
- Lirik bertimestamp dari [LRCLIB](https://lrclib.net) — baris yang sedang
  dinyanyikan tersorot, klik untuk melompat.
- Shortcut: `/` cari, spasi putar/jeda, ←/→ mundur/maju 5 detik, ↑/↓ volume,
  `n`/`p` lagu berikutnya/sebelumnya.
- Tema terang/gelap.

## Bangun

Butuh Rust 1.80+, `pkg-config` dan `alsa-lib` (header) untuk output audio.

```sh
cargo build --release
./target/release/swara
```

Pasang ke sistem:

```sh
sudo install -Dm755 target/release/swara /usr/local/bin/swara
```

Uji backend tanpa membuka jendela:

```sh
swara --probe "tulus monokrom"
```

Perintah itu mencari, mengambil URL stream, mengunduh, men-decode, lalu mencetak
durasi hasil decode, jumlah item antrean, lirik, dan isi beranda.

## Cara kerjanya

Metadata lewat klien InnerTube **WEB_REMIX** (`music.youtube.com/youtubei/v1`):
`search`, `browse`, `next`, `music/get_search_suggestions`. Kredensial
(`INNERTUBE_API_KEY`, versi klien, `visitorData`) diambil sekali dari halaman
utama dan disimpan 24 jam di `~/.cache/swara/creds.json`.

Stream lewat klien **VISIONOS** (client id 101) yang masih mengembalikan URL
langsung tanpa `signatureCipher` dan tanpa transformasi `n`. Kalau kelak klien
itu ditutup, ganti ke klien lain yang formatnya masih ber-url.

Audio diunduh utuh dengan header `Range: bytes=0-` — tanpa Range, googlevideo
menahan laju ke sekitar 40 KB/s dan koneksi menggantung. Setelah utuh, AAC
di-decode dengan `symphonia` lalu dikirim ke `rodio` sebagai
`Source<Item = f32>`.

`rodio::Decoder` sengaja tidak dipakai: pada stream AAC polos dari googlevideo,
rodio 0.19 memanggil `unreachable!` saat inisialisasi dan mematikan proses.
Seek dilakukan di tingkat sampel (buka ulang + lewati N frame), bukan lewat
index mp4 yang biasanya tidak ada di stream ini.

## Struktur

| berkas | isi |
|---|---|
| `src/api.rs` | klien InnerTube: kredensial, cari, beranda, browse, stream, radio, lirik |
| `src/player.rs` | dekoder AAC + rodio, seek berbasis sampel, volume, fade-in |
| `src/worker.rs` | thread HTTP supaya UI tidak membeku |
| `src/ui.rs` | seluruh antarmuka egui, palet dari sampul, pemutar |
| `src/main.rs` | entry point, `--probe` |

## Batasan

- Fitur akun (library, suka) belum ada; hanya butuh kata sandi Google, tidak
  mau menyentuh itu.
- Butuh internet, tentu saja.
- Antrean diambil sekali per lagu; tidak ada penyegaran berkala.

## Lisensi

MIT.
