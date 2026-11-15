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

use jni::objects::{GlobalRef, JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use std::sync::OnceLock;

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
