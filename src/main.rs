//! Swara — pemutar musik native untuk YouTube Music.
//!
//! `swara`             buka aplikasi
//! `swara --probe [q]` uji jalur InnerTube + unduh + decode tanpa membuka jendela

mod api;
mod auth;
mod bridge;
mod cepat;
mod conductor;
mod mpris;
mod stream;
mod theme;
mod player;
mod ui;
mod worker;

use api::Api;
use player::Player;
use std::sync::Arc;
use ui::App;
use worker::Worker;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("--probe") {
        let q = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "tulus monokrom".into());
        return probe(&q);
    }
    if args.iter().any(|a| a == "--login") {
        let api = Api::new();
        match api.login() {
            Ok(l) => {
                println!("masuk sebagai {l}");
                match api.library() {
                    Ok(v) => println!("koleksi: {} kartu", v.len()),
                    Err(e) => println!("koleksi gagal: {e:#}"),
                }
            }
            Err(e) => {
                println!("gagal masuk: {e:#}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--lirik") {
        let q = args[i + 1..].join(" ");
        let (artist, title) = match q.split_once(" - ") {
            Some((a, t)) => (a.to_string(), t.to_string()),
            None => (String::new(), q.clone()),
        };
        let api = Api::new();
        let l = api.lyrics(&title, &artist, 0.0);
        println!(
            "sinkron: {} baris, teks: {} baris",
            l.synced.len(),
            l.text.lines().count()
        );
        if let Some(f) = l.synced.first() {
            println!("contoh: {:.1}s {}", f.t, f.text);
        } else if let Some(f) = l.text.lines().next() {
            println!("contoh: {f}");
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("swara {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    // Daftar lagu yang disukai di akun (playlist "LM"). Dipakai untuk memilih
    // bahan uji yang benar-benar didengar pemiliknya.
    // URL audio mentah untuk satu video: bahan uji kecepatan unduh.
    if let Some(i) = args.iter().position(|a| a == "--url") {
        let id = args.get(i + 1).cloned().unwrap_or_default();
        let api = Api::new();
        let s = api.stream(&id)?;
        println!("{}", s.url);
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--cepat") {
        let id = args.get(i + 1).cloned().unwrap_or_default();
        cepat::uji(&id);
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--lib") {
        let api = Api::new();
        for c in api.library()? {
            println!("{} | {} | {}", c.id, c.title, c.kind);
        }
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--isi") {
        let id = args.get(i + 1).cloned().unwrap_or_default();
        let n = args
            .get(i + 2)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(30);
        let api = Api::new();
        let c = api.browse(&id)?;
        println!("{} — {} lagu", c.title, c.tracks.len());
        for t in c.tracks.iter().take(n) {
            println!("  {} | {} | {} | {}", t.id, t.title, t.artist, t.dur);
        }
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--suka") {
        let n = args
            .get(i + 1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(20);
        let api = Api::new();
        // "LM" sering ditolak (400) oleh klien ini; cari id playlist suka dari
        // daftar pustaka sebagai gantinya.
        let c = match api.browse("LM") {
            Ok(c) => c,
            Err(_) => {
                let id = api
                    .library()?
                    .into_iter()
                    .find(|k| {
                        let t = k.title.to_lowercase();
                        t.contains("disukai") || t.contains("liked")
                    })
                    .map(|k| k.id)
                    .unwrap_or_else(|| "VLLM".into());
                api.browse(&id)?
            }
        };
        println!("{} — {} lagu", c.title, c.tracks.len());
        for t in c.tracks.iter().take(n) {
            println!("  {} | {} | {} | {}", t.id, t.title, t.artist, t.dur);
        }
        return Ok(());
    }

    // Argumen bebas selain flag diperlakukan sebagai kueri: `swara "tulus"`.
    let autoplay = args.iter().find(|a| !a.starts_with('-')).cloned();
    // `SWARA_PLAY=<videoId>` memutar satu lagu tertentu tanpa lewat pencarian.
    let play_id = std::env::var("SWARA_PLAY").ok().filter(|s| !s.is_empty());

    let api = Arc::new(Api::new());
    let mut player = Player::new()?;
    let remote = player.remote();
    let (snap, ends) = player.watch();
    let worker = Worker::new(api.clone());
    let api2 = api.clone();
    // Konduktor dan jembatan MPRIS hidup sebelum jendela dibuka: keduanya
    // harus tetap jalan saat `update` berhenti (jendela di workspace lain).
    let (cond, cond_ctl) = conductor::spawn(api2.clone(), remote.clone(), snap.clone(), ends.clone());
    let mpris = mpris::Mpris::spawn();
    let mpris_ctl = bridge::spawn(mpris, remote.clone(), cond_ctl, cond.clone(), snap.clone());
    eprintln!("[swara] mulai, mpris={}", mpris_ctl.is_some());

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Swara")
            .with_app_id("swara"),
        ..Default::default()
    };

    eframe::run_native(
        "Swara",
        opts,
        Box::new(move |cc| {
            // Detak dari luar jendela. Kalau jendela disembunyikan (pindah
            // workspace), compositor berhenti mengirim frame callback sehingga
            // egui tidak pernah `update()` — lagu habis tapi antrean tidak
            // lanjut. `Context::request_repaint` dari thread lain membangunkan
            // event loop tanpa menunggu compositor.
            {
                let ctx = cc.egui_ctx.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    ctx.request_repaint();
                });
            }
            // Konduktor dan jembatan MPRIS sudah jalan di `main`; UI hanya
            // menerima pegangannya.
            Ok(Box::new(App::new(
                cc,
                api,
                player,
                worker,
                autoplay.clone(),
                play_id.clone(),
                mpris_ctl,
                cond,
            )) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Jalur lengkap tanpa GUI: cari, ambil URL, unduh, lalu decode. Dipakai untuk
/// memastikan backend benar-benar bekerja sebelum UI dipercaya.
fn probe(q: &str) -> anyhow::Result<()> {
    let api = Api::new();
    let t0 = std::time::Instant::now();
    let hasil = api.search(q, 5)?;
    println!(
        "search {:?} -> {} hasil ({} ms)",
        q,
        hasil.len(),
        t0.elapsed().as_millis()
    );
    for t in &hasil {
        println!("  {} | {} | {} | {}", t.id, t.title, t.artist, t.dur);
    }
    let first = hasil
        .first()
        .ok_or_else(|| anyhow::anyhow!("tidak ada hasil"))?
        .clone();

    let t1 = std::time::Instant::now();
    let s = api.stream(&first.id)?;
    println!(
        "stream ok itag={} {} {} kbps dur={} ({} ms)",
        s.itag,
        s.mime,
        s.kbps,
        s.dur,
        t1.elapsed().as_millis()
    );

    let t2 = std::time::Instant::now();
    let bytes = api.fetch_audio(&s.url)?;
    println!(
        "audio {} KB dalam {} ms",
        bytes.len() / 1024,
        t2.elapsed().as_millis()
    );

    let t3 = std::time::Instant::now();
    let (n, total) = probe_decode(&bytes)?;
    println!(
        "decode ok: {} sampel stereo, total {:.1} detik ({} ms)",
        n / 2,
        total,
        t3.elapsed().as_millis()
    );

    let t4 = std::time::Instant::now();
    let radio = api.radio(&first.id, 8)?;
    println!("radio {} item ({} ms)", radio.len(), t4.elapsed().as_millis());

    let t5 = std::time::Instant::now();
    let l = api.lyrics(&first.title, &first.artist, s.dur);
    println!(
        "lirik: {} baris bertimestamp, {} karakter teks ({} ms)",
        l.synced.len(),
        l.text.chars().count(),
        t5.elapsed().as_millis()
    );

    let t6 = std::time::Instant::now();
    let home = api.home()?;
    println!(
        "beranda: {} baris, {} kartu ({} ms)",
        home.len(),
        home.iter().map(|r| r.items.len()).sum::<usize>(),
        t6.elapsed().as_millis()
    );
    Ok(())
}

/// Decode penuh pakai jalur yang sama dengan pemutar (bukan `rodio::Decoder`,
/// yang panik pada AAC polos googlevideo).
fn probe_decode(bytes: &[u8]) -> anyhow::Result<(u64, f64)> {
    let mss = symphonia::core::io::MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        Default::default(),
    );
    let mut hint = symphonia::core::probe::Hint::new();
    hint.with_extension("m4a");
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &symphonia::core::formats::FormatOptions::default(),
        &symphonia::core::meta::MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow::anyhow!("tanpa track"))?;
    let id = track.id;
    let rate = track.codec_params.sample_rate.unwrap_or(44100) as f64;
    let mut dec = symphonia::default::get_codecs()
        .make(&track.codec_params, &symphonia::core::codecs::DecoderOptions::default())?;
    let mut samples = 0u64;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != id {
            continue;
        }
        if let Ok(buf) = dec.decode(&packet) {
            samples += (buf.frames() * 2) as u64;
        }
    }
    Ok((samples, samples as f64 / 2.0 / rate))
}
