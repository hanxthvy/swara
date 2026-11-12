//! Pemutar yang berjalan di pod sendiri.
//!
//! `rodio` dan `symphonia` dipakai langsung, bukan lewat `rodio::Decoder`,
//! karena stream googlevideo (itag 140) datang sebagai AAC polos: rodio 0.19
//! memanggil `unreachable!` saat init mendapat `SeekError`, sehingga program
//! yang mengandalkannya mati tanpa pesan. Di sini AAC di-decode sendiri dan
//! dikirim ke output sebagai `Source<Item = f32>`.

use anyhow::{anyhow, Result};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;
use symphonia::core::audio::{AudioBufferRef, Channels, Signal, SignalSpec};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

enum Cmd {
    /// (bytes audio, posisi mulai dalam detik, durasi dari metadata)
    Play(Vec<u8>, f64, f64),
    Pause,
    Resume,
    Stop,
    Seek(f64),
    Volume(f32),
}

#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub playing: bool,
    pub pos: f64,
    pub dur: f64,
    pub volume: f32,
    pub ended: bool,
    pub error: Option<String>,
}

/// Pegangan untuk mengendalikan pemutar dari thread lain (konduktor).
#[derive(Clone)]
pub struct Remote {
    tx: Sender<Cmd>,
}

impl Remote {
    pub fn play(&self, bytes: Vec<u8>, start: f64, dur: f64) {
        let _ = self.tx.send(Cmd::Play(bytes, start, dur));
    }
    pub fn pause(&self) {
        let _ = self.tx.send(Cmd::Pause);
    }
    pub fn resume(&self) {
        let _ = self.tx.send(Cmd::Resume);
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

pub struct Player {
    _stream: OutputStream,
    tx: Sender<Cmd>,
    rx: Receiver<Progress>,
    mirror: Option<Receiver<Progress>>,
    cur: Progress,
    played: Arc<AtomicU64>,
    rate: Arc<AtomicU64>,
}

impl Player {
    pub fn new() -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| anyhow!("output audio: {e}"))?;
        let (tx, cmd_rx) = channel::<Cmd>();
        let (prog_tx, prog_rx) = channel::<Progress>();
        // Salinan status untuk konduktor: ia yang menentukan lagu berikutnya,
        // jadi ia perlu tahu kapan lagu habis tanpa menunggu UI digambar.
        let (mirror_tx, mirror_rx) = channel::<Progress>();
        let played = Arc::new(AtomicU64::new(0));
        let rate = Arc::new(AtomicU64::new(48000));
        let (p2, r2) = (played.clone(), rate.clone());
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || audio_thread(handle, cmd_rx, prog_tx, mirror_tx, p2, r2))?;
        Ok(Player {
            _stream: stream,
            tx,
            rx: prog_rx,
            mirror: Some(mirror_rx),
            cur: Progress {
                volume: 0.8,
                ..Default::default()
            },
            played,
            rate,
        })
    }

    pub fn remote(&self) -> Remote {
        Remote { tx: self.tx.clone() }
    }

    /// Diambil sekali oleh konduktor.
    pub fn take_mirror(&mut self) -> Option<Receiver<Progress>> {
        self.mirror.take()
    }

    pub fn play(&mut self, bytes: Vec<u8>, start_at: f64, dur: f64) {
        self.played.store(0, Ordering::Relaxed);
        self.cur = Progress {
            volume: self.cur.volume,
            dur,
            pos: start_at,
            ..Default::default()
        };
        let _ = self.tx.send(Cmd::Play(bytes, start_at, dur));
    }

    pub fn pause(&mut self) {
        let _ = self.tx.send(Cmd::Pause);
    }
    pub fn resume(&mut self) {
        let _ = self.tx.send(Cmd::Resume);
    }
    pub fn stop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
    }
    pub fn seek(&mut self, pos: f64) {
        let secs = pos.max(0.0);
        let rate = self.rate.load(Ordering::Relaxed).max(1);
        self.played
            .store((secs * rate as f64) as u64 * 2, Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Seek(secs));
    }
    pub fn set_volume(&mut self, v: f32) {
        let _ = self.tx.send(Cmd::Volume(v));
    }

    /// Status terbaru. `ended` hanya true sekali per lagu.
    pub fn poll(&mut self) -> Progress {
        let mut ended_once = false;
        while let Ok(p) = self.rx.try_recv() {
            if p.ended && !self.cur.ended {
                ended_once = true;
            }
            self.cur = p;
        }
        let mut out = self.cur.clone();
        out.ended = ended_once;
        if ended_once {
            self.cur.ended = false;
        }
        out
    }
}

// ---------------------------------------------------------------- dekoder

struct Audio {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    spec: SignalSpec,
    total: Option<f64>,
}

fn open_audio(bytes: &[u8]) -> Result<Audio> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("m4a");
    hint.with_extension("mp4");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| anyhow!("probe: {e}"))?;
    let format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow!("tidak ada track audio"))?;
    let track_id = track.id;
    let rate = track.codec_params.sample_rate.unwrap_or(44100);
    let total = track.codec_params.n_frames.map(|n| n as f64 / rate as f64);
    let channels = track
        .codec_params
        .channels
        .unwrap_or(Channels::FRONT_LEFT | Channels::FRONT_RIGHT);
    let decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| anyhow!("codec: {e}"))?;
    Ok(Audio {
        format,
        decoder,
        track_id,
        spec: SignalSpec::new(rate, channels),
        total,
    })
}

/// Sumber sampel f32: paket AAC di-decode satu per satu, disimpan di antrean
/// supaya `next()` selalu murah. Selalu stereo.
struct AacSource {
    audio: Audio,
    pending: VecDeque<f32>,
    rate: u32,
    total: Option<Duration>,
    played: Arc<AtomicU64>,
    done: bool,
}

impl AacSource {
    fn new(audio: Audio, played: Arc<AtomicU64>) -> Self {
        let rate = audio.spec.rate;
        let total = audio.total.map(Duration::from_secs_f64);
        AacSource {
            audio,
            pending: VecDeque::with_capacity(16384),
            rate,
            total,
            played,
            done: false,
        }
    }

    fn fill(&mut self) -> bool {
        if self.done {
            return false;
        }
        loop {
            let packet = match self.audio.format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    self.done = true;
                    return false;
                }
                Err(SymphoniaError::ResetRequired) => continue,
                Err(_) => {
                    self.done = true;
                    return false;
                }
            };
            if packet.track_id() != self.audio.track_id {
                continue;
            }
            let decoded = match self.audio.decoder.decode(&packet) {
                Ok(d) => d,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => {
                    self.done = true;
                    return false;
                }
            };
            // Buffer hasil decode meminjam decoder (field `self`). Salin ke
            // Vec dulu supaya pinjaman lepas sebelum antrean disentuh.
            let samples = interleave(&decoded);
            drop(decoded);
            self.pending.extend(samples);
            return true;
        }
    }

    /// Buang sampel pertama sebanyak `frames` frame (seek tanpa container).
    fn skip_frames(&mut self, frames: u64) {
        let mut need = frames * 2;
        while need > 0 {
            if self.pending.is_empty() && !self.fill() {
                return;
            }
            let take = (self.pending.len() as u64).min(need) as usize;
            self.pending.drain(..take);
            need -= take as u64;
        }
    }
}

impl Iterator for AacSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(s) = self.pending.pop_front() {
            self.played.fetch_add(1, Ordering::Relaxed);
            return Some(s);
        }
        if self.fill() {
            self.next()
        } else {
            None
        }
    }
}

impl Source for AacSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.total
    }
}

/// Ubah satu buffer hasil decode menjadi sampel stereo terinterleaved.
fn interleave(buf: &AudioBufferRef<'_>) -> Vec<f32> {
    let ch = buf.spec().channels.count();
    let frames = buf.frames();
    let mono = ch == 1;
    let mut out = Vec::with_capacity(frames * 2);
    macro_rules! stereo {
        ($b:expr, $conv:expr) => {{
            for i in 0..frames {
                let l = $conv($b.chan(0)[i]);
                let r = if mono { l } else { $conv($b.chan(1)[i]) };
                out.push(l);
                out.push(r);
            }
        }};
    }
    match buf {
        AudioBufferRef::F32(b) => stereo!(b, |v: f32| v),
        AudioBufferRef::S16(b) => stereo!(b, |v: i16| v as f32 / 32768.0),
        AudioBufferRef::S32(b) => stereo!(b, |v: i32| v as f32 / 2147483648.0),
        AudioBufferRef::S24(b) => {
            stereo!(b, |v: symphonia::core::sample::i24| v.inner() as f32 / 8388608.0)
        }
        AudioBufferRef::U8(b) => stereo!(b, |v: u8| (v as f32 - 128.0) / 128.0),
        other => {
            let mut sb =
                symphonia::core::audio::SampleBuffer::<f32>::new(frames as u64, *other.spec());
            sb.copy_interleaved_ref(other.clone());
            for pair in sb.samples().chunks(2) {
                let l = pair[0];
                let r = *pair.get(1).unwrap_or(&l);
                out.push(l);
                out.push(r);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- thread

struct Cur {
    sink: Sink,
    dur: f64,
    playing: bool,
}

fn audio_thread(
    handle: OutputStreamHandle,
    rx: Receiver<Cmd>,
    tx: Sender<Progress>,
    mirror: Sender<Progress>,
    played: Arc<AtomicU64>,
    rate_slot: Arc<AtomicU64>,
) {
    let mut cur: Option<Cur> = None;
    let mut volume = 0.8f32;
    let mut error: Option<String> = None;
    let mut last_bytes: Vec<u8> = Vec::new();
    let mut last_dur = 0.0f64;
    // Posisi dihitung dari jam dinding, bukan dari jumlah sampel yang sudah
    // ditarik sink. Hitungan sampel meleset kalau perangkat menegosiasikan laju
    // lain (PipeWire/easyeffects), dan lirik jadi makin jauh tiap menit.
    let mut base_pos = 0.0f64;
    let mut started: Option<std::time::Instant> = None;
    // Sink yang sudah kosong tetap "kosong" sampai Play berikutnya datang.
    // Tanpa penanda ini `ended` dilaporkan tiap poll, dan pemanggil melompat
    // lagu berulang-ulang selama jeda muat (gejala: skip beruntun).
    let mut reported_end = false;

    loop {
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Cmd::Play(bytes, start, dur)) => {
                if let Some(c) = cur.take() {
                    c.sink.stop();
                }
                reported_end = false;
                error = None;
                last_dur = dur;
                last_bytes = bytes;
                base_pos = start.max(0.0);
                started = Some(std::time::Instant::now());
                match build(&last_bytes, start, &handle, volume, dur, &played, &rate_slot) {
                    Ok(sink) => {
                        cur = Some(Cur {
                            sink,
                            dur,
                            playing: true,
                        })
                    }
                    Err(e) => error = Some(e),
                }
            }
            Ok(Cmd::Pause) => {
                if let Some(t) = started.take() {
                    base_pos += t.elapsed().as_secs_f64();
                }
                if let Some(c) = cur.as_mut() {
                    c.sink.pause();
                    c.playing = false;
                }
            }
            Ok(Cmd::Resume) => {
                if let Some(c) = cur.as_mut() {
                    c.sink.play();
                    c.playing = true;
                    started = Some(std::time::Instant::now());
                }
            }
            Ok(Cmd::Stop) => {
                if let Some(c) = cur.as_mut() {
                    c.sink.stop();
                }
                cur = None;
                started = None;
            }
            Ok(Cmd::Seek(pos)) => {
                // Seek di tingkat sampel: buang audio lama, buka ulang, lalu
                // lewati `pos` detik. Tidak butuh index mp4 yang sering tidak
                // ada pada stream googlevideo.
                if let Some(c) = cur.take() {
                    c.sink.stop();
                }
                reported_end = false;
                base_pos = pos.max(0.0);
                started = Some(std::time::Instant::now());
                if !last_bytes.is_empty() {
                    match build(
                        &last_bytes,
                        pos,
                        &handle,
                        volume,
                        last_dur,
                        &played,
                        &rate_slot,
                    ) {
                        Ok(sink) => {
                            error = None;
                            cur = Some(Cur {
                                sink,
                                dur: last_dur,
                                playing: true,
                            })
                        }
                        Err(e) => error = Some(e),
                    }
                }
            }
            Ok(Cmd::Volume(v)) => {
                volume = v;
                if let Some(c) = cur.as_mut() {
                    c.sink.set_volume(v);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let status = match cur.as_mut() {
            Some(c) => {
                let _rate = rate_slot.load(Ordering::Relaxed).max(1);
                let pos = base_pos
                    + started
                        .map(|t| t.elapsed().as_secs_f64())
                        .unwrap_or(0.0);
                let pos = pos.max(0.0);
                let empty = c.playing && c.sink.empty();
                let ended = empty && error.is_none() && !reported_end;
                if ended {
                    reported_end = true;
                    c.playing = false;
                }
                Progress {
                    playing: c.playing && !empty,
                    pos: if c.dur > 0.0 { pos.min(c.dur) } else { pos },
                    dur: c.dur,
                    volume,
                    ended,
                    error: error.clone(),
                }
            }
            None => Progress {
                playing: false,
                pos: 0.0,
                dur: 0.0,
                volume,
                ended: false,
                error: error.clone(),
            },
        };
        if tx.send(status.clone()).is_err() {
            return;
        }
        // Konduktor boleh tertinggal; jangan sampai pemutaran ikut berhenti.
        let _ = mirror.send(status);
    }
}

#[allow(clippy::too_many_arguments)]
fn build(
    bytes: &[u8],
    start: f64,
    handle: &OutputStreamHandle,
    volume: f32,
    dur: f64,
    played: &Arc<AtomicU64>,
    rate_slot: &Arc<AtomicU64>,
) -> Result<Sink, String> {
    let audio = open_audio(bytes).map_err(|e| format!("{e}"))?;
    let rate = audio.spec.rate as u64;
    rate_slot.store(rate, Ordering::Relaxed);
    played.store(0, Ordering::Relaxed);
    let mut src = AacSource::new(audio, played.clone());
    let frames = (start.max(0.0) * rate as f64) as u64;
    if frames > 0 {
        src.skip_frames(frames);
    }
    played.store(frames * 2, Ordering::Relaxed);
    let _ = dur;
    if src.pending.is_empty() && !src.fill() {
        return Err("audio tidak bisa didekode".into());
    }
    let sink = Sink::try_new(handle).map_err(|e| format!("sink: {e}"))?;
    sink.set_volume(volume);
    sink.append(src.fade_in(Duration::from_millis(160)));
    Ok(sink)
}
