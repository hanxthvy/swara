//! Swara — pemutar musik native untuk YouTube Music.
//!
//! Semua modul hidup di sini supaya bisa dipakai dua cara: binari CLI
//! (`src/main.rs`) dan aplikasi Android (`cdylib` yang dipanggil lewat
//! `android_main`).

pub mod api;
pub mod auth;
#[cfg(target_os = "linux")]
pub mod ctl;
#[cfg(not(target_os = "linux"))]
#[path = "ctl_stub.rs"]
pub mod ctl;
pub mod cepat;
pub mod conductor;
pub mod paths;
pub mod player;
pub mod stream;
pub mod theme;
pub mod ui;
pub mod worker;

// MPRIS itu D-Bus, jadi hanya ada di Linux. Di sistem lain dipakai pengganti
// tanpa efek supaya sisa kode tidak perlu bercabang.
#[cfg(target_os = "linux")]
pub mod bridge;
#[cfg(target_os = "linux")]
pub mod mpris;
#[cfg(not(target_os = "linux"))]
#[path = "bridge_stub.rs"]
pub mod bridge;
#[cfg(not(target_os = "linux"))]
#[path = "mpris_stub.rs"]
pub mod mpris;

#[cfg(target_os = "android")]
pub mod jembatan_android;

use api::Api;
use player::Player;
use std::sync::Arc;
use ui::App;
use worker::Worker;

/// Permintaan awal saat jendela dibuka.
#[derive(Default, Clone)]
pub struct Launch {
    pub autoplay: Option<String>,
    pub play_id: Option<String>,
    pub play_list: Option<String>,
    #[cfg(target_os = "android")]
    pub android_app: Option<android_activity::AndroidApp>,
}

/// Buka jendela utama. Blok sampai jendela ditutup.
pub fn run_gui(launch: Launch) -> anyhow::Result<()> {
    let api = Arc::new(Api::new());
    let mut player = Player::new()?;
    let remote = player.remote();
    #[cfg(target_os = "android")]
    {
        // Tombol dari kartu pemutar bawaan Android harus tetap bekerja saat
        // jendela tidak menggambar di latar.
        let (snap2, _) = player.watch();
        jembatan_android::pasang_kendali(remote.clone(), snap2);
    }
    let (snap, ends) = player.watch();
    let worker = Worker::new(api.clone());
    let api2 = api.clone();
    // Konduktor dan jembatan MPRIS hidup sebelum jendela dibuka: keduanya
    // harus tetap jalan saat `update` berhenti (jendela di workspace lain).
    let (cond, cond_ctl) = conductor::spawn(api2.clone(), remote.clone(), snap.clone(), ends.clone());
    let mpris = mpris::Mpris::spawn();
    let mpris_ctl = bridge::spawn(mpris, remote.clone(), cond_ctl, cond.clone(), snap.clone());
    // Kendali luar (islet): tombol suka di pill notch memakai soket ini, dan
    // berkas keadaannya dibaca islet tanpa polling.
    let suka = ctl::bersama();
    ctl::spawn(worker.pintu(), suka.clone());
    eprintln!("[swara] mulai, mpris={}", mpris_ctl.is_some());

    let mut opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Swara")
            .with_app_id("swara"),
        ..Default::default()
    };
    // Di Android Activity diserahkan lewat hook event loop, bukan NativeOptions.
    #[cfg(target_os = "android")]
    if let Some(app) = launch.android_app.clone() {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        opts.event_loop_builder = Some(Box::new(move |b| {
            b.with_android_app(app);
        }));
    }

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
            Ok(Box::new(App::new(
                cc,
                api,
                player,
                worker,
                launch.autoplay.clone(),
                launch.play_id.clone(),
                launch.play_list.clone(),
                mpris_ctl,
                cond,
                suka,
            )) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Titik masuk Android: dipanggil oleh `android-activity` begitu Activity siap.
#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: android_activity::AndroidApp) {
    // Satu proses hanya boleh punya satu loop UI. Kalau proses lama masih
    // hidup (dulu mungkin karena layanan START_STICKY), panggilan kedua akan
    // membuat dua loop berebut AudioFlinger dan berakhir crash.
    static MASUK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if MASUK.swap(true, std::sync::atomic::Ordering::SeqCst) {
        eprintln!("[swara] android_main dipanggil dua kali di proses ini; keluar");
        return;
    }
    eprintln!("[swara] android_main masuk");
    // Berkas Swara disimpan di dalam direktori privat aplikasi, bukan HOME.
    if let Some(dir) = app.internal_data_path() {
        paths::set_android_dirs(dir.clone(), dir.join("cache"));
    }
    eprintln!("[swara] path siap");
    jembatan_android::simpan_handle(&app);
    eprintln!("[swara] jni siap");
    jembatan_android::mulai_layanan_pemutaran();
    eprintln!("[swara] layanan diminta");
    let mut launch = Launch::default();
    launch.android_app = Some(app);
    if let Err(e) = run_gui(launch) {
        eprintln!("[swara] gagal membuka jendela: {e:#}");
    }
    // Jendela sudah ditutup (activity dihancurkan). Layanan foreground akan
    // menjaga proses ini hidup selamanya kalau tidak dimatikan dari sini.
    jembatan_android::hentikan_proses();
}
