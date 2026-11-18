//! Jembatan JNI ke sisi Java: layanan pemutaran latar dan halaman login WebView.
//!
//! Di Android tidak ada D-Bus dan tidak ada profil Chromium, jadi dua hal yang
//! di desktop ditangani MPRIS dan `swara --login` di sini ditangani Activity
//! Java: `Latar` menjaga proses tetap hidup saat layar mati, `Masuk` membuka
//! WebView untuk login lalu menulis cookie ke berkas.
//!
//! Kelas Java dicari lewat `ClassLoader` milik aplikasi, bukan `FindClass`:
//! dari thread native, `FindClass` hanya tahu classpath sistem dan berakhir
//! dengan `ClassNotFoundException`.

use crate::player::{Progress, Remote};
use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Aksi tombol dari notifikasi/layar kunci, ditampung dulu lalu dijalankan UI.
static AKSI: Mutex<Vec<i32>> = Mutex::new(Vec::new());

pub const AKSI_TOGGLE: i32 = 0;
pub const AKSI_NEXT: i32 = 1;
pub const AKSI_PREV: i32 = 2;
pub const AKSI_PAUSE: i32 = 3;
pub const AKSI_PLAY: i32 = 4;
pub const AKSI_SUKA: i32 = 5;

/// Pegangan pemutar untuk tombol dari kartu bawaan Android. Dipakai saat
/// jendela sedang tidak menggambar (aplikasi di latar, layar mati): egui tidak
/// memanggil `update`, jadi aksi yang hanya ditumpuk di antrean tidak akan
/// pernah dijalankan sampai aplikasi dibuka lagi.
static REMOTE: OnceLock<Remote> = OnceLock::new();
static SNAP: OnceLock<Arc<Mutex<Progress>>> = OnceLock::new();

/// Detak terakhir `App::update`, dalam mikrodetik sejak proses mulai.
static DETAK: AtomicU64 = AtomicU64::new(0);
static AWAL: OnceLock<Instant> = OnceLock::new();

fn sekarang_us() -> u64 {
    AWAL.get_or_init(Instant::now).elapsed().as_micros() as u64
}

/// Dipanggil UI setiap giliran gambar: tanda jendela masih hidup.
pub fn detak() {
    DETAK.store(sekarang_us(), Ordering::Relaxed);
}

pub fn pasang_kendali(remote: Remote, snap: Arc<Mutex<Progress>>) {
    let _ = REMOTE.set(remote);
    let _ = SNAP.set(snap);
}

/// Jendela dianggap hidup kalau ada detak dalam 1,2 detik terakhir.
fn ui_hidup() -> bool {
    let d = DETAK.load(Ordering::Relaxed);
    d != 0 && sekarang_us().saturating_sub(d) < 1_200_000
}

fn main_sekarang() -> bool {
    SNAP.get()
        .and_then(|s| s.lock().ok().map(|g| g.playing))
        .unwrap_or(false)
}

fn jalankan_langsung(aksi: i32) -> bool {
    let (Some(r), Some(_)) = (REMOTE.get(), SNAP.get()) else {
        return false;
    };
    eprintln!("[swara] kendali langsung aksi={aksi} main={}", main_sekarang());
    match aksi {
        AKSI_PAUSE => r.pause(),
        AKSI_PLAY => r.resume(),
        AKSI_TOGGLE => {
            if main_sekarang() {
                r.pause();
            } else {
                r.resume();
            }
        }
        _ => return false,
    }
    true
}

static VM: OnceLock<JavaVM> = OnceLock::new();
static AKTIVITAS: OnceLock<GlobalRef> = OnceLock::new();
static PEMUAT: OnceLock<GlobalRef> = OnceLock::new();

/// Dipanggil sekali di `android_main`, sebelum apa pun yang butuh Java.
pub fn simpan_handle(app: &android_activity::AndroidApp) {
    unsafe {
        if let Ok(vm) = JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) {
            let _ = VM.set(vm);
        }
    }
    dengan_env(|e| unsafe {
        let akt = JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject);
        if let Ok(g) = e.new_global_ref(&akt) {
            let _ = AKTIVITAS.set(g);
        }
        // ClassLoader aplikasi, dipakai untuk menemukan kelas Swara dari thread ini.
        if let Ok(cl) = e.call_method(&akt, "getClassLoader", "()Ljava/lang/ClassLoader;", &[]) {
            if let Ok(obj) = cl.l() {
                if let Ok(g) = e.new_global_ref(&obj) {
                    let _ = PEMUAT.set(g);
                }
            }
        }
    });
}

/// Jalankan sesuatu dengan `JNIEnv` yang sudah terpasang di thread ini.
fn dengan_env<T>(f: impl FnOnce(&mut JNIEnv) -> T) -> Option<T> {
    let vm = VM.get()?;
    let mut guard = vm.attach_current_thread().ok()?;
    let env: &mut JNIEnv = &mut guard;
    Some(f(env))
}

/// Cari kelas Swara lewat ClassLoader aplikasi.
fn kelas<'a>(e: &mut JNIEnv<'a>, nama: &str) -> Option<JClass<'a>> {
    let pemuat = PEMUAT.get()?;
    let nama = e.new_string(nama).ok()?;
    let hasil = e
        .call_method(
            pemuat.as_obj(),
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&nama)],
        )
        .ok()?;
    if e.exception_check().unwrap_or(false) {
        let _ = e.exception_clear();
        return None;
    }
    let obj: JObject<'a> = hasil.l().ok()?;
    Some(JClass::from(obj))
}

fn panggil(kelas_java: &str, nama: &str) {
    dengan_env(|e| {
        let Some(akt) = AKTIVITAS.get() else { return };
        let Some(k) = kelas(e, kelas_java) else {
            eprintln!("[swara] kelas {kelas_java} tidak ketemu");
            return;
        };
        if let Err(err) = e.call_static_method(
            &k,
            nama,
            "(Landroid/content/Context;)V",
            &[JValue::Object(akt.as_obj())],
        ) {
            eprintln!("[swara] {kelas_java}.{nama} gagal: {err}");
            if e.exception_check().unwrap_or(false) {
                let _ = e.exception_describe();
                let _ = e.exception_clear();
            }
        }
    });
}

/// Nyalakan foreground service supaya pemutaran tidak dibunuh saat layar mati.
pub fn mulai_layanan_pemutaran() {
    panggil("com.hanxthvy.swara.Latar", "mulai");
}

/// Buka halaman login WebView. Hasilnya dibaca lewat [`baca_cookie`].
pub fn buka_login() {
    panggil("com.hanxthvy.swara.Masuk", "buka");
}

/// Cookie yang sudah ditulis halaman login, dalam bentuk header `Cookie`.
pub fn baca_cookie() -> Option<String> {
    dengan_env(|e| {
        let akt = AKTIVITAS.get()?;
        let k = kelas(e, "com.hanxthvy.swara.Masuk")?;
        let hasil = e
            .call_static_method(
                &k,
                "cookie",
                "(Landroid/content/Context;)Ljava/lang/String;",
                &[JValue::Object(akt.as_obj())],
            )
            .ok()?;
        if e.exception_check().unwrap_or(false) {
            let _ = e.exception_clear();
            return None;
        }
        let obj = hasil.l().ok()?;
        if obj.is_null() {
            return None;
        }
        let jstr = JString::from(obj);
        let teks: String = e.get_string(&jstr).ok()?.into();
        if teks.trim().is_empty() {
            None
        } else {
            Some(teks)
        }
    })
    .flatten()
}

/// Dipanggil Java dari sesi media: tombol di notifikasi, layar kunci, atau
/// headset. Hanya menampung; UI yang menjalankannya supaya jalurnya sama
/// dengan tombol di layar.
#[no_mangle]
pub extern "system" fn Java_com_hanxthvy_swara_Pemutar_kendaliNative(
    _env: *mut jni::sys::JNIEnv,
    _kelas: jni::sys::jclass,
    aksi: jni::sys::jint,
) {
    // Saat jendela tidak menggambar, jeda/lanjut dikerjakan langsung di sini.
    // Aksi lain (berikutnya/sebelumnya, suka) tetap lewat UI karena butuh
    // antrean dan sesi akun.
    if !ui_hidup() && jalankan_langsung(aksi) {
        return;
    }
    if let Ok(mut g) = AKSI.lock() {
        g.push(aksi);
    }
}

/// Ambil aksi yang menumpuk sejak frame terakhir.
pub fn ambil_aksi() -> Vec<i32> {
    AKSI.lock().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
}

/// Tulis keadaan pemutaran ke sesi media Android (notifikasi + layar kunci).
/// Dibatasi supaya tidak memanggil Java 60 kali per detik untuk posisi.
pub fn perbarui_media(
    judul: &str,
    artis: &str,
    sampul: &str,
    suka: bool,
    main: bool,
    pos: f64,
    dur: f64,
) {
    static TERAKHIR: Mutex<(String, String, String, bool, bool, Option<std::time::Instant>)> =
        Mutex::new((String::new(), String::new(), String::new(), false, false, None));
    let perlu = {
        let Ok(mut g) = TERAKHIR.lock() else { return };
        let berubah = g.0 != judul || g.1 != artis || g.2 != sampul || g.3 != suka || g.4 != main;
        let basi = g.5.map(|t| t.elapsed().as_millis() >= 900).unwrap_or(true);
        if berubah || basi {
            *g = (
                judul.to_string(),
                artis.to_string(),
                sampul.to_string(),
                suka,
                main,
                Some(std::time::Instant::now()),
            );
            true
        } else {
            false
        }
    };
    if !perlu {
        return;
    }
    dengan_env(|e| {
        let Some(akt) = AKTIVITAS.get() else { return };
        let Some(k) = kelas(e, "com.hanxthvy.swara.Pemutar") else {
            eprintln!("[swara] kelas Pemutar tidak ketemu");
            return;
        };
        let (Ok(j), Ok(a), Ok(u)) = (
            e.new_string(judul),
            e.new_string(artis),
            e.new_string(sampul),
        ) else {
            return;
        };
        let hasil = e.call_static_method(
            &k,
            "perbarui",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZJJ)V",
            &[
                JValue::Object(akt.as_obj()),
                JValue::Object(&j),
                JValue::Object(&a),
                JValue::Object(&u),
                JValue::Bool(u8::from(suka)),
                JValue::Bool(u8::from(main)),
                JValue::Long((pos.max(0.0) * 1000.0) as i64),
                JValue::Long((dur.max(0.0) * 1000.0) as i64),
            ],
        );
        if let Err(err) = hasil {
            eprintln!("[swara] Pemutar.perbarui gagal: {err}");
            if e.exception_check().unwrap_or(false) {
                let _ = e.exception_describe();
                let _ = e.exception_clear();
            }
        }
    });
}

/// Tinggi bilah status sistem, dalam piksel fisik. Nol kalau tidak terbaca.
pub fn inset_atas_px() -> f32 {
    dengan_env(|e| {
        let Some(akt) = AKTIVITAS.get() else { return 0.0 };
        let Some(k) = kelas(e, "com.hanxthvy.swara.Layar") else {
            return 0.0;
        };
        let hasil = match e.call_static_method(
            &k,
            "insetAtas",
            "(Landroid/content/Context;)F",
            &[JValue::Object(akt.as_obj())],
        ) {
            Ok(v) => v,
            Err(_) => {
                let _ = e.exception_clear();
                return 0.0;
            }
        };
        hasil.f().unwrap_or(0.0)
    })
    .unwrap_or(0.0)
}

/// Activity sudah dihancurkan: hentikan layanan latar lalu matikan proses.
/// Tanpa ini, layanan foreground menjaga proses tetap hidup setelah kartu
/// aplikasi di-swipe dari daftar recent.
pub fn hentikan_proses() {
    dengan_env(|e| {
        let Some(akt) = AKTIVITAS.get() else { return };
        let Some(k) = kelas(e, "com.hanxthvy.swara.Latar") else { return };
        if let Err(err) = e.call_static_method(
            &k,
            "berhenti",
            "(Landroid/content/Context;)V",
            &[JValue::Object(akt.as_obj())],
        ) {
            eprintln!("[swara] Latar.berhenti gagal: {err}");
            let _ = e.exception_clear();
        }
    });
    std::process::exit(0);
}
