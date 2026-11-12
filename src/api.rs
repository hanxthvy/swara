//! InnerTube client: pencarian, beranda, halaman koleksi, stream audio, lirik.
//!
//! Cari dan metadata lewat klien WEB_REMIX (butuh kredensial + visitorData).
//! Stream lewat klien VISIONOS (client id 101) yang masih memberi URL langsung
//! tanpa signatureCipher dan tanpa transformasi `n`.

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const UA_WEB: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
pub const UA_VIS: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15";
const API: &str = "https://music.youtube.com/youtubei/v1/";

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub dur: String,
    pub secs: f64,
    pub thumb: String,
}

#[derive(Clone, Debug, Default)]
pub struct Card {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub sub: String,
    pub thumb: String,
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub title: String,
    pub items: Vec<Card>,
}

#[derive(Clone, Debug, Default)]
pub struct Collection {
    pub id: String,
    pub title: String,
    pub sub: String,
    pub thumb: String,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Default)]
pub struct LyricLine {
    pub t: f64,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct Lyrics {
    pub synced: Vec<LyricLine>,
    pub text: String,
    pub footer: String,
}

#[derive(Clone, Debug, Default)]
pub struct Stream {
    pub url: String,
    pub itag: i64,
    pub mime: String,
    pub kbps: u64,
    pub title: String,
    pub artist: String,
    pub dur: f64,
    pub thumb: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Creds {
    key: String,
    ver: String,
    vd: String,
    ts: u64,
}

pub struct Api {
    client: reqwest::blocking::Client,
    creds: Mutex<Option<Creds>>,
}

fn cache_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".cache").join("swara")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn collect<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            if let Some(x) = m.get(key) {
                out.push(x);
            }
            for val in m.values() {
                collect(val, key, out);
            }
        }
        Value::Array(a) => {
            for val in a {
                collect(val, key, out);
            }
        }
        _ => {}
    }
}

fn items<'a>(v: &'a Value, key: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    collect(v, key, &mut out);
    out
}

/// teks dari `{ runs: [...] }` atau `{ simpleText }`.
fn runs_text(o: &Value) -> String {
    if let Some(s) = o.as_str() {
        return s.to_string();
    }
    let mut s = String::new();
    if let Some(rs) = o.get("runs").and_then(|r| r.as_array()) {
        for r in rs {
            s.push_str(r.get("text").and_then(|t| t.as_str()).unwrap_or(""));
        }
    }
    if s.is_empty() {
        if let Some(t) = o.get("simpleText").and_then(|t| t.as_str()) {
            s = t.to_string();
        }
    }
    s
}

/// teks dari flexColumn musicResponsiveListItemFlexColumnRenderer.
fn col_text(col: &Value) -> String {
    let t = col
        .get("musicResponsiveListItemFlexColumnRenderer")
        .and_then(|c| c.get("text"))
        .cloned()
        .unwrap_or(Value::Null);
    runs_text(&t)
}

fn best_thumb(thumbs: &Value) -> String {
    let arr = match thumbs.as_array() {
        Some(a) => a,
        None => return String::new(),
    };
    let re = Regex::new(r"=w(\d+)-h(\d+)").unwrap();
    let mut best = "";
    let mut best_w = -1i64;
    for t in arr {
        let u = t.get("url").and_then(|u| u.as_str()).unwrap_or("");
        let w = re
            .captures(u)
            .and_then(|c| c.get(1).and_then(|m| m.as_str().parse::<i64>().ok()))
            .unwrap_or(0);
        if w > best_w {
            best_w = w;
            best = u;
        }
    }
    let strip = Regex::new(r"=w\d+-h\d+.*$").unwrap();
    strip.replace(best, "=w544-h544-l90-rj").to_string()
}

/// "3:24" -> detik
fn parse_dur(s: &str) -> f64 {
    let mut total = 0f64;
    for part in s.split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().unwrap_or(0.0);
    }
    total
}

impl Api {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent(UA_WEB)
            .timeout(Duration::from_secs(40))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        let cached = fs::read(cache_dir().join("creds.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Creds>(&b).ok());
        Api {
            client,
            creds: Mutex::new(cached),
        }
    }

    fn creds(&self) -> Result<Creds> {
        let mut lock = self.creds.lock().unwrap();
        if let Some(c) = lock.as_ref() {
            if c.ts + 86_400 > now() && !c.key.is_empty() {
                return Ok(c.clone());
            }
        }
        let html = self
            .client
            .get("https://music.youtube.com/")
            .send()?
            .text()?;
        let grab = |pat: &str| -> Option<String> {
            Regex::new(pat)
                .unwrap()
                .captures(&html)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        };
        let c = Creds {
            key: grab(r#""INNERTUBE_API_KEY":"([^"]+)""#).unwrap_or_default(),
            ver: grab(r#""INNERTUBE_CONTEXT_CLIENT_VERSION":"([^"]+)""#).unwrap_or_default(),
            vd: grab(r#""VISITOR_DATA":"([^"]+)""#).unwrap_or_default(),
            ts: now(),
        };
        if c.key.is_empty() || c.ver.is_empty() || c.vd.is_empty() {
            return Err(anyhow!("kredensial InnerTube gagal diambil"));
        }
        let _ = fs::create_dir_all(cache_dir());
        let _ = fs::write(cache_dir().join("creds.json"), serde_json::to_vec(&c)?);
        *lock = Some(c.clone());
        Ok(c)
    }

    fn ctx_remix(c: &Creds) -> Value {
        json!({"client": {
            "clientName": "WEB_REMIX", "clientVersion": c.ver,
            "hl": "id", "gl": "ID", "visitorData": c.vd,
        }})
    }

    fn itube(&self, ep: &str, body: Value) -> Result<Value> {
        let c = self.creds()?;
        let url = format!("{API}{ep}?key={}&prettyPrint=false", c.key);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Youtube-Client-Name", "67")
            .header("X-Youtube-Client-Version", &c.ver)
            .header("X-Goog-Visitor-Id", &c.vd)
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .json(&body)
            .send()?;
        if !resp.status().is_success() {
            return Err(anyhow!("{} -> {}", ep, resp.status()));
        }
        Ok(resp.json()?)
    }

    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<Track>> {
        let c = self.creds()?;
        let body = json!({
            "context": Self::ctx_remix(&c),
            "query": q,
            "params": "EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D",
        });
        let resp = self.itube("search", body)?;
        let mut out: Vec<Track> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for it in items(&resp, "musicResponsiveListItemRenderer") {
            let vid = match it
                .get("playlistItemData")
                .and_then(|p| p.get("videoId"))
                .and_then(|v| v.as_str())
            {
                Some(v) => v.to_string(),
                None => continue,
            };
            if !seen.insert(vid.clone()) {
                continue;
            }
            let fc = it.get("flexColumns").and_then(|f| f.as_array()).cloned().unwrap_or_default();
            let title = fc.first().map(|c| col_text(c)).unwrap_or_default();
            let byline = fc.get(1).map(|c| col_text(c)).unwrap_or_default();
            let mut parts: Vec<String> = byline.split('•').map(|p| p.trim().to_string()).collect();
            let dur_re = Regex::new(r"^\d{1,2}[.:]\d{2}$").unwrap();
            let mut dur = String::new();
            if let Some(last) = parts.last() {
                if dur_re.is_match(last) {
                    dur = parts.pop().unwrap().replace('.', ":");
                }
            }
            let artist = parts.first().cloned().unwrap_or_default();
            let album = parts.get(1).cloned().unwrap_or_default();
            let secs = parse_dur(&dur);
            out.push(Track {
                id: vid,
                title,
                artist,
                album,
                dur,
                secs,
                thumb: best_thumb(
                    it.get("thumbnail")
                        .and_then(|t| t.get("musicThumbnailRenderer"))
                        .and_then(|m| m.get("thumbnail"))
                        .and_then(|t| t.get("thumbnails"))
                        .unwrap_or(&Value::Null),
                ),
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    pub fn suggest(&self, q: &str) -> Result<Vec<String>> {
        let c = self.creds()?;
        let resp = self.itube(
            "music/get_search_suggestions",
            json!({"context": Self::ctx_remix(&c), "input": q}),
        )?;
        let mut out: Vec<String> = Vec::new();
        for it in items(&resp, "searchSuggestionRenderer") {
            let t = it.get("suggestion").map(runs_text).unwrap_or_default();
            if !t.is_empty() && !out.contains(&t) {
                out.push(t);
            }
        }
        out.truncate(8);
        Ok(out)
    }

    pub fn stream(&self, video_id: &str) -> Result<Stream> {
        let c = self.creds()?;
        let body = json!({
            "context": {"client": {
                "clientName": "VISIONOS", "clientVersion": "1.02",
                "deviceMake": "Apple", "deviceModel": "RealityDevice17,1",
                "osName": "visionOS", "osVersion": "26.5.23O471",
                "hl": "id", "gl": "ID", "visitorData": c.vd,
            }},
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true,
        });
        let resp: Value = self
            .client
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("Content-Type", "application/json")
            .header("User-Agent", UA_VIS)
            .header("X-Youtube-Client-Name", "101")
            .header("X-Youtube-Client-Version", "1.02")
            .header("X-Goog-Visitor-Id", &c.vd)
            .header("X-Goog-Api-Format-Version", "1")
            .header("Origin", "https://www.youtube.com")
            .json(&body)
            .send()?
            .json()?;

        let status = resp
            .pointer("/playabilityStatus/status")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        if status != "OK" {
            let reason = resp
                .pointer("/playabilityStatus/reason")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            return Err(anyhow!("{status}: {reason}"));
        }

        let mut formats: Vec<&Value> = resp
            .pointer("/streamingData/adaptiveFormats")
            .and_then(|f| f.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        formats.retain(|f| {
            f.get("mimeType").and_then(|m| m.as_str()).unwrap_or("").starts_with("audio")
                && f.get("url").is_some()
        });
        if formats.is_empty() {
            return Err(anyhow!("tidak ada format audio ber-url"));
        }
        // Prioritaskan AAC dalam mp4: rodio mendekodenya lewat symphonia tanpa
        // perlu container webm.
        formats.sort_by_key(|f| {
            let mime = f.get("mimeType").and_then(|m| m.as_str()).unwrap_or("");
            let is_mp4 = mime.contains("mp4");
            let br = f.get("bitrate").and_then(|b| b.as_u64()).unwrap_or(0);
            (is_mp4 as u64, br)
        });
        let f = formats.last().unwrap();
        let vd = resp.get("videoDetails").cloned().unwrap_or(Value::Null);
        let thumb = best_thumb(
            resp.pointer("/microformat/microformatDataRenderer/thumbnail/thumbnails")
                .or_else(|| vd.pointer("/thumbnail/thumbnails"))
                .unwrap_or(&Value::Null),
        );
        let author = vd.get("author").and_then(|a| a.as_str()).unwrap_or("");
        let artist = Regex::new(r"\s*-?\s*Topic$")
            .unwrap()
            .replace(author, "")
            .to_string();
        Ok(Stream {
            url: f.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string(),
            itag: f.get("itag").and_then(|i| i.as_i64()).unwrap_or(0),
            mime: f.get("mimeType").and_then(|m| m.as_str()).unwrap_or("").to_string(),
            kbps: f.get("bitrate").and_then(|b| b.as_u64()).unwrap_or(0) / 1000,
            title: vd.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string(),
            artist: artist.trim().to_string(),
            dur: vd
                .get("lengthSeconds")
                .and_then(|l| l.as_str())
                .and_then(|l| l.parse::<f64>().ok())
                .unwrap_or(0.0),
            thumb,
        })
    }

    pub fn radio(&self, video_id: &str, limit: usize) -> Result<Vec<Track>> {
        let c = self.creds()?;
        let resp = self.itube(
            "next",
            json!({
                "context": Self::ctx_remix(&c),
                "videoId": video_id,
                "isAudioOnly": true,
                "playlistId": format!("RDAMVM{video_id}"),
            }),
        )?;
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        seen.insert(video_id.to_string());
        for it in items(&resp, "playlistPanelVideoRenderer") {
            let vid = it
                .get("videoId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    it.pointer("/navigationEndpoint/watchEndpoint/videoId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                });
            let vid = match vid {
                Some(v) => v,
                None => continue,
            };
            if !seen.insert(vid.clone()) {
                continue;
            }
            let dur = it
                .get("lengthText")
                .map(|l| runs_text(l))
                .unwrap_or_default();
            let artist = it
                .get("longBylineText")
                .map(|l| runs_text(l))
                .unwrap_or_default()
                .split('•')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            out.push(Track {
                id: vid,
                title: it.get("title").map(|t| runs_text(t)).unwrap_or_default(),
                artist,
                album: String::new(),
                secs: parse_dur(&dur),
                dur,
                thumb: best_thumb(
                    it.get("thumbnail")
                        .and_then(|t| t.get("thumbnails"))
                        .unwrap_or(&Value::Null),
                ),
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    fn card(node: &Value) -> Option<Card> {
        if let Some(two) = node.get("musicTwoRowItemRenderer") {
            let nav = |n: &Value| -> (String, String) {
                if let Some(we) = n.pointer("/navigationEndpoint/watchEndpoint/videoId") {
                    return ("song".into(), we.as_str().unwrap_or("").to_string());
                }
                if let Some(be) = n.pointer("/navigationEndpoint/browseEndpoint/browseId") {
                    return ("browse".into(), be.as_str().unwrap_or("").to_string());
                }
                if let Some(v) = n.get("videoId").and_then(|v| v.as_str()) {
                    return ("song".into(), v.to_string());
                }
                (String::new(), String::new())
            };
            let (mut kind, mut id) = nav(two);
            if id.is_empty() {
                if let Some(ov) = two.pointer(
                    "/thumbnailOverlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint",
                ) {
                    let (k, i) = nav(ov);
                    kind = k;
                    id = i;
                }
            }
            let thumb = best_thumb(
                two.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails")
                    .or_else(|| {
                        two.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
                    })
                    .unwrap_or(&Value::Null),
            );
            return Some(Card {
                kind,
                id,
                title: two.get("title").map(runs_text).unwrap_or_default(),
                sub: two.get("subtitle").map(runs_text).unwrap_or_default(),
                thumb,
            });
        }
        if let Some(resp) = node.get("musicResponsiveListItemRenderer") {
            let fc = resp
                .get("flexColumns")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            let mut id = resp
                .pointer("/playlistItemData/videoId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let mut kind = "song".to_string();
            if id.is_empty() {
                if let Some(we) = resp.pointer("/navigationEndpoint/watchEndpoint/videoId") {
                    id = we.as_str().unwrap_or("").to_string();
                    kind = "song".into();
                } else if let Some(be) = resp.pointer("/navigationEndpoint/browseEndpoint/browseId") {
                    id = be.as_str().unwrap_or("").to_string();
                    kind = "browse".into();
                }
            }
            let sub = fc
                .iter()
                .skip(1)
                .map(col_text)
                .collect::<Vec<_>>()
                .join(" • ");
            return Some(Card {
                kind,
                id,
                title: fc.first().map(|c| col_text(c)).unwrap_or_default(),
                sub,
                thumb: best_thumb(
                    resp.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
                        .unwrap_or(&Value::Null),
                ),
            });
        }
        None
    }

    pub fn home(&self) -> Result<Vec<Row>> {
        let c = self.creds()?;
        let resp = self.itube(
            "browse",
            json!({"context": Self::ctx_remix(&c), "browseId": "FEmusic_home"}),
        )?;
        let mut rows = Vec::new();
        for sh in items(&resp, "musicCarouselShelfRenderer") {
            let title = sh
                .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
                .map(runs_text)
                .unwrap_or_default();
            let mut cards: Vec<Card> = Vec::new();
            for node in sh
                .get("contents")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default()
            {
                if let Some(card) = Self::card(&node) {
                    if !card.id.is_empty() {
                        cards.push(card);
                    }
                }
            }
            cards.truncate(16);
            if !cards.is_empty() {
                rows.push(Row { title, items: cards });
            }
        }
        Ok(rows)
    }

    pub fn browse(&self, browse_id: &str) -> Result<Collection> {
        let c = self.creds()?;
        let resp = self.itube(
            "browse",
            json!({"context": Self::ctx_remix(&c), "browseId": browse_id}),
        )?;
        let mut head = Value::Null;
        for key in [
            "musicDetailHeaderRenderer",
            "musicResponsiveHeaderRenderer",
            "musicEditablePlaylistDetailHeaderRenderer",
        ] {
            if let Some(h) = items(&resp, key).first() {
                head = (*h).clone();
                break;
            }
        }
        let mut title = String::new();
        let mut thumb = String::new();
        let mut sub = String::new();
        if !head.is_null() {
            title = head.get("title").map(runs_text).unwrap_or_default();
            thumb = best_thumb(
                head.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
                    .or_else(|| {
                        head.pointer("/thumbnail/croppedSquareThumbnailRenderer/thumbnail/thumbnails")
                    })
                    .unwrap_or(&Value::Null),
            );
            sub = head.get("subtitle").map(runs_text).unwrap_or_default();
            if sub.is_empty() {
                sub = head.get("secondSubtitle").map(runs_text).unwrap_or_default();
            }
        }
        let mut tracks = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for node in items(&resp, "musicResponsiveListItemRenderer") {
            let n = json!({"musicResponsiveListItemRenderer": node});
            if let Some(card) = Self::card(&n) {
                if card.id.is_empty() || !seen.insert(card.id.clone()) {
                    continue;
                }
                let fc = node
                    .get("flexColumns")
                    .and_then(|f| f.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut artist = String::new();
                let mut dur = String::new();
                if let Some(second) = fc.get(1) {
                    let parts: Vec<String> = col_text(second)
                        .split('•')
                        .map(|p| p.trim().to_string())
                        .collect();
                    artist = parts.first().cloned().unwrap_or_default();
                    dur = parts.last().cloned().unwrap_or_default();
                    if !Regex::new(r"^\d{1,2}[.:]\d{2}$").unwrap().is_match(&dur) {
                        dur.clear();
                    }
                }
                tracks.push(Track {
                    id: card.id,
                    title: card.title,
                    artist,
                    album: String::new(),
                    secs: parse_dur(&dur),
                    dur,
                    thumb: card.thumb,
                });
            }
        }
        if title.is_empty() {
            title = browse_id.to_string();
        }
        Ok(Collection {
            id: browse_id.to_string(),
            title,
            sub,
            thumb,
            tracks,
        })
    }

    pub fn lyrics(&self, title: &str, artist: &str, secs: f64) -> Lyrics {
        let mut out = Lyrics::default();
        if let Some(synced) = self.lrclib(title, artist, secs) {
            out.synced = synced;
        }
        out
    }

    fn lrclib(&self, title: &str, artist: &str, duration: f64) -> Option<Vec<LyricLine>> {
        if title.is_empty() {
            return None;
        }
        let q = format!("{} {}", artist, title);
        let url = reqwest::Url::parse_with_params("https://lrclib.net/api/search", &[("q", q.as_str())]).ok()?;
        let resp = self
            .client
            .get(url)
            .header("User-Agent", "swara/0.1 (local player)")
            .send()
            .ok()?;
        let data: Value = resp.json().ok()?;
        let arr = data.as_array()?;
        let mut best: Option<(f64, &Value)> = None;
        for c in arr {
            let synced = match c.get("syncedLyrics").and_then(|s| s.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let _ = synced;
            let d = c.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
            let mut dist = (d - duration).abs();
            if !artist.is_empty() {
                let a: String = artist.to_lowercase().chars().take(6).collect();
                let name = c
                    .get("artistName")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                if !name.contains(&a) {
                    dist += 30.0;
                }
            }
            if best.map(|(bd, _)| dist < bd).unwrap_or(true) {
                best = Some((dist, c));
            }
        }
        let (dist, cand) = best?;
        if dist > 6.0 {
            return None;
        }
        let lines = parse_lrc(cand.get("syncedLyrics").and_then(|s| s.as_str()).unwrap_or(""));
        if lines.is_empty() {
            None
        } else {
            Some(lines)
        }
    }

    /// Ambil audio utuh. Range `bytes=0-` wajib: tanpa Range googlevideo
    /// menahan laju ke ~40 KB/s dan koneksinya menggantung.
    pub fn fetch_audio(&self, url: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get(url)
            .header("User-Agent", UA_VIS)
            .header("Range", "bytes=0-")
            .header("Accept-Encoding", "identity")
            .timeout(Duration::from_secs(120))
            .send()?;
        if !resp.status().is_success() && resp.status().as_u16() != 206 {
            return Err(anyhow!("stream http {}", resp.status()));
        }
        let bytes = resp.bytes()?.to_vec();
        if bytes.len() < 4096 {
            return Err(anyhow!("audio terlalu kecil ({} B)", bytes.len()));
        }
        Ok(bytes)
    }

    pub fn fetch_image(&self, url: &str) -> Result<Vec<u8>> {
        if url.is_empty() {
            return Err(anyhow!("url kosong"));
        }
        let resp = self.client.get(url).send()?;
        Ok(resp.bytes()?.to_vec())
    }
}

pub fn thumb_or(u: &str) -> Option<String> {
    if u.is_empty() {
        None
    } else {
        Some(u.to_string())
    }
}

fn parse_lrc(txt: &str) -> Vec<LyricLine> {
    let stamp = Regex::new(r"\[(\d{1,2}):(\d{2})(?:[.:](\d{1,3}))?\]").unwrap();
    let mut out: Vec<LyricLine> = Vec::new();
    for raw in txt.lines() {
        let caps: Vec<_> = stamp.captures_iter(raw).collect();
        if caps.is_empty() {
            continue;
        }
        let text = stamp.replace_all(raw, "").trim().to_string();
        if text.is_empty() {
            continue;
        }
        for c in caps {
            let m: f64 = c.get(1).unwrap().as_str().parse().unwrap_or(0.0);
            let s: f64 = c.get(2).unwrap().as_str().parse().unwrap_or(0.0);
            let frac = c
                .get(3)
                .map(|f| {
                    let d = f.as_str();
                    d.parse::<f64>().unwrap_or(0.0) / 10f64.powi(d.len() as i32)
                })
                .unwrap_or(0.0);
            out.push(LyricLine {
                t: m * 60.0 + s + frac,
                text: text.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (a.t - b.t).abs() < 0.01 && a.text == b.text);
    out
}
