//! Sesi akun dari Chromium.
//!
//! Swara tidak pernah meminta kata sandi Google. Yang dipakai adalah cookie
//! YouTube yang sudah ada di profil Chromium di mesin ini: nilai `v10`
//! didekripsi dengan AES-128-CBC (kunci PBKDF2-SHA1 dari "peanuts", 1 iterasi,
//! IV 16 spasi), lalu 32 byte hash host dibuang dari depan. Setelah itu
//! `Authorization: SAPISIDHASH` dihitung dari SAPISID seperti yang dilakukan
//! web YouTube sendiri.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Session {
    pub profile: String,
    /// user: nama yang ditampilkan di UI
    pub label: String,
    pub cookies: BTreeMap<String, String>,
    pub saved_at: u64,
}

fn cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".cache").join("swara").join("auth.json")
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn candidates() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let base = PathBuf::from(home);
    let mut out = Vec::new();
    for rel in [
        ".config/chromium",
        ".config/google-chrome",
        ".config/chromium-browser",
        ".config/BraveSoftware/Brave-Browser",
        ".config/microsoft-edge",
        ".config/vivaldi",
    ] {
        let dir = base.join(rel);
        if dir.is_dir() {
            let def = dir.join("Default");
            out.push(if def.is_dir() { def } else { dir });
        }
    }
    out
}

pub fn load() -> Option<Session> {
    let b = std::fs::read(cache_path()).ok()?;
    let s: Session = serde_json::from_slice(&b).ok()?;
    if s.cookies.is_empty() {
        return None;
    }
    Some(s)
}

pub fn save(s: &Session) -> Result<()> {
    let p = cache_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, serde_json::to_vec(s)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn clear() {
    let _ = std::fs::remove_file(cache_path());
}

fn key_peanuts() -> [u8; 16] {
    let mut out = [0u8; 16];
    pbkdf2::pbkdf2::<hmac::Hmac<Sha1>>(b"peanuts", b"saltysalt", 1, &mut out)
        .expect("pbkdf2");
    out
}

fn decrypt_v10(blob: &[u8], key: &[u8; 16]) -> Option<String> {
    if blob.len() <= 3 {
        return None;
    }
    let mut buf = blob[3..].to_vec();
    let iv = [b' '; 16];
    let dec = Aes128CbcDec::new(key.into(), &iv.into());
    let plain = dec.decrypt_padded_mut::<Pkcs7>(&mut buf).ok()?;
    // Chromium >= v24 menempelkan sha256(host_key) 32 byte di depan.
    let body = if plain.len() > 32 { &plain[32..] } else { plain };
    let s = String::from_utf8_lossy(body).trim_end_matches('\0').to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Ambil cookie youtube.com/google.com dari profil Chromium di `dir`.
pub fn read_profile(dir: &Path) -> Result<BTreeMap<String, String>> {
    let db = [dir.join("Cookies"), dir.join("Network").join("Cookies")]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!("Cookies tidak ada di {}", dir.display()))?;
    // Chromium memegang lock; salin ke berkas sementara.
    // TMPDIR bisa menunjuk ke direktori milik pengguna lain (mis. sesi root),
    // jadi salinan dibuat di cache milik pengguna sendiri.
    let Some(tmpdir) = cache_path().parent().map(|p| p.to_path_buf()) else {
        bail!("cache swara tidak diketahui");
    };
    std::fs::create_dir_all(&tmpdir)?;
    let tmp = tmpdir.join(format!("cookies-{}.db", std::process::id()));
    std::fs::copy(&db, &tmp).map_err(|e| anyhow!("salin {}: {e}", db.display()))?;
    let con = rusqlite::Connection::open_with_flags(
        &tmp,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut stmt = con.prepare(
        "select host_key, name, encrypted_value, value from cookies \
         where host_key like '%youtube.com%' or host_key like '%google.com%'",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Vec<u8>>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let key = key_peanuts();
    let mut jar: BTreeMap<String, (String, String)> = BTreeMap::new();
    for row in rows.flatten() {
        let (host, name, enc, plain) = row;
        let value = if !plain.is_empty() {
            plain
        } else if enc.starts_with(b"v10") {
            match decrypt_v10(&enc, &key) {
                Some(v) => v,
                None => continue,
            }
        } else {
            String::new()
        };
        if value.is_empty() {
            continue;
        }
        // Domain yang lebih spesifik menang; cukup satu nilai per nama.
        let mut keep = true;
        if let Some((old_host, _)) = jar.get(&name) {
            keep = host.len() > old_host.len();
        }
        if keep {
            jar.insert(name, (host, value));
        }
    }
    drop(stmt);
    drop(con);
    let _ = std::fs::remove_file(&tmp);
    Ok(jar.into_iter().map(|(k, (_, v))| (k, v)).collect())
}

/// Muat sesi dari profil Chromium pertama yang punya cookie login.
pub fn import_any() -> Result<Session> {
    let mut last = None;
    for dir in candidates() {
        match read_profile(&dir) {
            Ok(jar) => {
                if jar.contains_key("SAPISID") || jar.contains_key("__Secure-3PAPISID") {
                    let s = Session {
                        profile: dir.display().to_string(),
                        label: String::new(),
                        cookies: jar,
                        saved_at: now(),
                    };
                    save(&s)?;
                    return Ok(s);
                }
                last = Some(format!("{}: tanpa sesi YouTube", dir.display()));
            }
            Err(e) => last = Some(format!("{e}")),
        }
    }
    Err(anyhow!(
        "{}",
        last.unwrap_or_else(|| "tidak ada profil Chromium ditemukan".into())
    ))
}

pub fn cookie_header(s: &Session) -> String {
    s.cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Header `Authorization` gaya web YouTube.
pub fn auth_headers(s: &Session) -> Vec<(String, String)> {
    let sapisid = s
        .cookies
        .get("SAPISID")
        .or_else(|| s.cookies.get("__Secure-3PAPISID"))
        .cloned()
        .unwrap_or_default();
    if sapisid.is_empty() {
        return Vec::new();
    }
    let ts = now();
    let mut h = Sha1::new();
    h.update(format!("{ts} {sapisid} https://music.youtube.com").as_bytes());
    let digest: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    vec![
        ("Cookie".into(), cookie_header(s)),
        ("Authorization".into(), format!("SAPISIDHASH {ts}_{digest}")),
        ("X-Goog-AuthUser".into(), "0".into()),
        ("X-Origin".into(), "https://music.youtube.com".into()),
    ]
}
