//! Uji cepat: klien mana yang memberi URL googlevideo paling responsif.
//! Dipakai untuk memilih klien di `api::stream` — bukan bagian pemutaran.

use serde_json::{json, Value};
use std::io::Read;
use std::time::{Duration, Instant};

pub struct Klien {
    pub nama: &'static str,
    pub head: [&'static str; 3], // X-Youtube-Client-Name, -Version, UA
    pub ctx: Value,
}

pub fn semua() -> Vec<Klien> {
    vec![
        Klien {
            nama: "VISIONOS",
            head: ["101", "1.02", crate::api::UA_VIS],
            ctx: json!({"client": {"clientName": "VISIONOS", "clientVersion": "1.02",
                "deviceMake": "Apple", "deviceModel": "RealityDevice17,1",
                "osName": "visionOS", "osVersion": "26.5.23O471", "hl": "id", "gl": "ID"}}),
        },
        Klien {
            nama: "ANDROID_VR",
            head: ["28", "1.62.27", "com.google.android.apps.youtube.vr.oculus/1.62.27 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip"],
            ctx: json!({"client": {"clientName": "ANDROID_VR", "clientVersion": "1.62.27",
                "deviceMake": "Oculus", "deviceModel": "Quest 3", "androidSdkVersion": 32,
                "osName": "Android", "osVersion": "12L", "hl": "id", "gl": "ID"}}),
        },
        Klien {
            nama: "IOS",
            head: ["5", "19.45.4", "com.google.ios.youtube/19.45.4 (iPhone16,2; U; CPU iOS 18_1_0 like Mac OS X;)"],
            ctx: json!({"client": {"clientName": "IOS", "clientVersion": "19.45.4",
                "deviceMake": "Apple", "deviceModel": "iPhone16,2", "osName": "iPhone",
                "osVersion": "18.1.0.22B83", "hl": "id", "gl": "ID"}}),
        },
        Klien {
            nama: "TV_EMBED",
            head: ["85", "2.0", "Mozilla/5.0 (PlayStation; PlayStation 4/12.00) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.0 Safari/605.1.15"],
            ctx: json!({"client": {"clientName": "TVHTML5_SIMPLY_EMBEDDED_PLAYER",
                "clientVersion": "2.0", "hl": "id", "gl": "ID"}}),
        },
        Klien {
            nama: "WEB",
            head: ["1", "2.20241126.01.00", crate::api::UA_WEB],
            ctx: json!({"client": {"clientName": "WEB", "clientVersion": "2.20241126.01.00",
                "hl": "id", "gl": "ID"}}),
        },
    ]
}

/// Jalankan uji untuk satu video. Hasil: satu baris per klien.
pub fn uji(video_id: &str) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("http client");
    for k in semua() {
        let mut ctx = k.ctx.clone();
        ctx["client"]["visitorData"] = json!(crate::api::visitor_data());
        let body = json!({
            "context": ctx,
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true,
        });
        let t_api = Instant::now();
        let resp: Result<Value, _> = client
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("Content-Type", "application/json")
            .header("User-Agent", k.head[2])
            .header("X-Youtube-Client-Name", k.head[0])
            .header("X-Youtube-Client-Version", k.head[1])
            .header("Origin", "https://www.youtube.com")
            .json(&body)
            .send()
            .and_then(|r| r.json());
        let api_ms = t_api.elapsed().as_millis();
        let resp = match resp {
            Ok(v) => v,
            Err(e) => {
                println!("{:<12} player: gagal ({e})", k.nama);
                continue;
            }
        };
        let st = resp
            .pointer("/playabilityStatus/status")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let mut formats: Vec<&Value> = resp
            .pointer("/streamingData/adaptiveFormats")
            .and_then(|f| f.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        formats.retain(|f| {
            f.get("mimeType")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .starts_with("audio")
                && f.get("url").is_some()
        });
        if st != "OK" || formats.is_empty() {
            println!("{:<12} status={st} format={} api={api_ms}ms", k.nama, formats.len());
            continue;
        }
        formats.sort_by_key(|f| f.get("bitrate").and_then(|b| b.as_u64()).unwrap_or(0));
        let f = formats.last().unwrap();
        let url = f.get("url").and_then(|u| u.as_str()).unwrap_or("");
        let itag = f.get("itag").and_then(|i| i.as_i64()).unwrap_or(0);
        let (ttfb, speed, got) = ukur(&client, url, k.head[2]);
        println!(
            "{:<12} itag={:<4} api={api_ms}ms ttfb={ttfb}ms speed={:.0}KB/s ({}KB)",
            k.nama,
            itag,
            speed / 1024.0,
            got / 1024
        );
    }
}

/// Unduh sampai 1 MB atau 8 detik; kembalikan (ttfb ms, byte/s, byte).
fn ukur(client: &reqwest::blocking::Client, url: &str, ua: &str) -> (u128, f64, usize) {
    let t0 = Instant::now();
    let resp = client
        .get(url)
        .header("User-Agent", ua)
        .header("Range", "bytes=0-")
        .header("Accept-Encoding", "identity")
        .send();
    let mut resp = match resp {
        Ok(r) => r,
        Err(e) => return (0, 0.0, 0),
    };
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0usize;
    let mut ttfb = 0u128;
    while got < 1024 * 1024 && t0.elapsed() < Duration::from_secs(8) {
        match resp.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if ttfb == 0 {
                    ttfb = t0.elapsed().as_millis();
                }
                got += n;
            }
            Err(_) => break,
        }
    }
    let secs = t0.elapsed().as_secs_f64().max(0.001);
    (ttfb, got as f64 / secs, got)
}
