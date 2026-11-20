package com.hanxthvy.swara;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.media.AudioAttributes;
import android.media.AudioFocusRequest;
import android.media.AudioManager;
import android.media.MediaMetadata;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import android.os.Build;

/**
 * Sesi media Android: kartu pemutar bawaan sistem di notifikasi dan layar kunci,
 * plus permintaan fokus audio supaya Swara berhenti sendiri saat aplikasi lain
 * mulai memutar suara.
 *
 * Sisi Rust menulis keadaan lewat {@link #perbarui}, dan menerima tombolnya
 * sebagai aksi di {@code kendaliNative}. Aksi itu tidak langsung menyentuh
 * pemutar: UI yang menjalankannya, supaya jalurnya sama dengan tombol di layar.
 */
public class Player {
    public static final int AKSI_TOGGLE = 0;
    public static final int AKSI_NEXT = 1;
    public static final int AKSI_PREV = 2;
    public static final int AKSI_PAUSE = 3;
    public static final int AKSI_PLAY = 4;
    public static final int AKSI_SUKA = 5;

    private static final String KANAL = "swara_putar";
    private static final int ID = 1001;

    private static MediaSession sesi;
    private static NotificationManager nm;
    private static AudioManager am;
    private static AudioFocusRequest fokus;
    private static boolean pegangFokus = false;
    private static boolean sedangMain = false;
    private static String judul = "";
    private static String artis = "";
    private static Context app;
    private static boolean mintaNotif = false;
    private static boolean sukaSekarang = false;
    /** Durasi terakhir yang diketahui; dipakai saat lagu baru belum melapor. */
    private static long durTerakhir = 0;
    private static String urlSeni = "";
    private static android.graphics.Bitmap seni = null;

    static {
        // Tanpa ini, simbol Java_com_hanxthvy_swara_Pemutar_kendaliNative tidak
        // terdaftar untuk classloader aplikasi dan setiap tombol notifikasi
        // berakhir UnsatisfiedLinkError (aplikasi langsung mati).
        System.loadLibrary("swara");
    }

    /** Dipanggil Rust di setiap perubahan lagu/keadaan putar. */
    public static native void kendaliNative(int aksi);

    /** kendaliNative yang tidak boleh menjatuhkan aplikasi bila simbol hilang. */
    private static void kirim(int aksi) {
        try {
            kendaliNative(aksi);
        } catch (Throwable t) {
            android.util.Log.w("Swara", "kendali native gagal: " + t);
        }
    }

    private static final AudioManager.OnAudioFocusChangeListener pendengar =
            new AudioManager.OnAudioFocusChangeListener() {
                @Override
                public void onAudioFocusChange(int f) {
                    switch (f) {
                        case AudioManager.AUDIOFOCUS_LOSS:
                        case AudioManager.AUDIOFOCUS_LOSS_TRANSIENT:
                            // Aplikasi lain butuh suara. Jeda, bukan perkecil volume:
                            // perilaku pemutar musik pada umumnya.
                            pegangFokus = false;
                            kirim(AKSI_PAUSE);
                            break;
                        case AudioManager.AUDIOFOCUS_GAIN:
                            pegangFokus = true;
                            break;
                        default:
                            break;
                    }
                }
            };

    /**
     * @param j       judul lagu
     * @param a       artis
     * @param main    sedang memutar
     * @param posMs   posisi dalam milidetik
     * @param durMs   durasi dalam milidetik
     */
    public static void perbarui(Context c, String j, String a, String seniUrl, boolean suka,
                                boolean main, long posMs, long durMs) {
        app = c.getApplicationContext();
        siapkan(c);

        boolean bergantiLagu = !j.equals(judul) || !a.equals(artis);
        boolean bergantiMain = main != sedangMain;
        boolean bergantiSuka = suka != sukaSekarang;
        judul = j;
        artis = a;
        sedangMain = main;
        sukaSekarang = suka;

        // Sampul diunduh sendiri: MediaSession hanya menerima Bitmap, bukan URL.
        // Saat lagu berganti, sampul lama dilepas lebih dulu — kalau tidak,
        // gambar lagu sebelumnya menempel sampai notifikasi disegarkan lain.
        String url = seniUrl == null ? "" : seniUrl;
        if (bergantiLagu) {
            seni = null;
        }
        if (!url.equals(urlSeni) || bergantiLagu) {
            urlSeni = url;
            muatSeni(url);
        }

        if (durMs > 0) {
            durTerakhir = durMs;
        }
        // Tanpa durasi, Android menggambar state NONE dan bilah durasi di kartu
        // hilang. Pakai durasi terakhir selama lagu baru belum melaporkannya.
        boolean dapatDurasi = durMs > 0 && durMs != durTerakhir;
        long dur = durMs > 0 ? durMs : durTerakhir;
        perbaruiMetadata(dur);

        // STATE_NONE membuat kartu kehilangan bilah durasi, jadi selama masih
        // ada lagu keadaannya PLAYING atau PAUSED saja.
        int st = main ? PlaybackState.STATE_PLAYING : PlaybackState.STATE_PAUSED;
        PlaybackState.Builder pb = new PlaybackState.Builder()
                .setActions(PlaybackState.ACTION_PLAY
                        | PlaybackState.ACTION_PAUSE
                        | PlaybackState.ACTION_PLAY_PAUSE
                        | PlaybackState.ACTION_SKIP_TO_NEXT
                        | PlaybackState.ACTION_SKIP_TO_PREVIOUS
                        // Tanpa ini bilah durasi di kartu tidak bisa digeser.
                        | PlaybackState.ACTION_SEEK_TO)
                .setState(st, Math.max(0, posMs), main ? 1f : 0f);
        PlaybackState.CustomAction sukaAksi = aksiSuka(c);
        if (sukaAksi != null) {
            pb.addCustomAction(sukaAksi);
        }
        sesi.setPlaybackState(pb.build());
        sesi.setActive(true);

        if (main) {
            mintaFokus();
        } else if (pegangFokus) {
            lepasFokus();
        }

        // Notifikasi hanya disegarkan saat lagu/keadaan berubah; posisi cukup
        // lewat PlaybackState supaya bilah system UI tetap halus.
        if (bergantiLagu || bergantiMain || bergantiSuka || dapatDurasi) {
            tampilkan();
        }
    }

    /** Metadata lagu yang sedang main, termasuk sampul bila sudah terunduh. */
    private static void perbaruiMetadata(long durMs) {
        if (sesi == null) {
            return;
        }
        MediaMetadata.Builder m = new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, judul)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, artis);
        if (durMs > 0) {
            m.putLong(MediaMetadata.METADATA_KEY_DURATION, durMs);
        }
        if (seni != null) {
            m.putBitmap(MediaMetadata.METADATA_KEY_ALBUM_ART, seni);
            m.putBitmap(MediaMetadata.METADATA_KEY_ART, seni);
        }
        sesi.setMetadata(m.build());
    }

    /**
     * Unduh sampul di thread sendiri lalu pasang ke sesi. Tanpa ini kartu
     * pemutar bawaan Android tampil kosong tanpa gambar.
     */
    private static void muatSeni(final String url) {
        if (url == null || url.isEmpty()) {
            return;
        }
        final Context ctx = app;
        Thread t = new Thread(new Runnable() {
            @Override
            public void run() {
                android.graphics.Bitmap bmp = null;
                try {
                    java.net.HttpURLConnection h = (java.net.HttpURLConnection) new java.net.URL(url).openConnection();
                    h.setConnectTimeout(6000);
                    h.setReadTimeout(8000);
                    h.setRequestProperty("User-Agent", "Mozilla/5.0");
                    java.io.InputStream in = h.getInputStream();
                    byte[] isi = baca(in);
                    in.close();
                    bmp = android.graphics.BitmapFactory.decodeByteArray(isi, 0, isi.length);
                } catch (Throwable e) {
                    bmp = null;
                }
                if (bmp == null || !url.equals(urlSeni)) {
                    return;
                }
                seni = bmp;
                new android.os.Handler(android.os.Looper.getMainLooper()).post(new Runnable() {
                    @Override
                    public void run() {
                        perbaruiMetadata(0);
                        tampilkan();
                    }
                });
            }
        }, "seni-swara");
        t.setDaemon(true);
        t.start();
    }

    private static byte[] baca(java.io.InputStream in) throws java.io.IOException {
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        byte[] buf = new byte[8192];
        int n;
        while ((n = in.read(buf)) > 0) {
            out.write(buf, 0, n);
        }
        return out.toByteArray();
    }

    /** Tombol suka di kartu pemutar; ikonnya ikut keadaan sekarang. */
    private static PlaybackState.CustomAction aksiSuka(Context c) {
        try {
            String nama = sukaSekarang ? "ic_jempol_isi" : "ic_jempol";
            int id = c.getResources().getIdentifier(nama, "drawable", c.getPackageName());
            if (id == 0) {
                return null;
            }
            PlaybackState.CustomAction.Builder b = new PlaybackState.CustomAction.Builder(
                    sukaSekarang ? "swara.batal" : "swara.suka",
                    sukaSekarang ? "Batal suka" : "Suka",
                    id);
            return b.build();
        } catch (Throwable t) {
            return null;
        }
    }

    private static void siapkan(Context c) {
        nm = (NotificationManager) c.getSystemService(Context.NOTIFICATION_SERVICE);
        if (Build.VERSION.SDK_INT >= 26) {
            NotificationChannel ch = new NotificationChannel(KANAL, "Pemutaran", NotificationManager.IMPORTANCE_LOW);
            ch.setShowBadge(false);
            nm.createNotificationChannel(ch);
        }
        if (sesi == null) {
            sesi = new MediaSession(c, "Swara");
            sesi.setFlags(MediaSession.FLAG_HANDLES_MEDIA_BUTTONS | MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS);
            sesi.setCallback(new MediaSession.Callback() {
                @Override
                public void onPlay() {
                    kirim(AKSI_PLAY);
                }

                @Override
                public void onPause() {
                    kirim(AKSI_PAUSE);
                }

                @Override
                public void onSkipToNext() {
                    kirim(AKSI_NEXT);
                }

                @Override
                public void onSkipToPrevious() {
                    kirim(AKSI_PREV);
                }

                @Override
                public void onStop() {
                    kirim(AKSI_PAUSE);
                }

                @Override
                public void onSeekTo(long pos) {
                    kendaliSeek(pos);
                }

                @Override
                public void onCustomAction(String action, android.os.Bundle args) {
                    if (action != null && action.startsWith("swara.suka")) {
                        kirim(AKSI_SUKA);
                    }
                }
            },
            // Thread UI Swara milik Rust tidak punya Looper; tanpa Handler
            // main thread, setCallback langsung melempar RuntimeException.
            new android.os.Handler(android.os.Looper.getMainLooper()));
        }
        mintaIzinNotifikasi(c);
    }

    /** Geser posisi dari kartu sistem / layar kunci. */
    private static void kendaliSeek(long ms) {
        try {
            kendaliSeekNative(ms);
        } catch (Throwable t) {
            android.util.Log.w("Swara", "seek native gagal: " + t);
        }
    }

    private static native void kendaliSeekNative(long ms);

    private static void mintaIzinNotifikasi(Context c) {
        if (Build.VERSION.SDK_INT < 33 || mintaNotif) {
            return;
        }
        if (c.checkSelfPermission("android.permission.POST_NOTIFICATIONS") == PackageManager.PERMISSION_GRANTED) {
            return;
        }
        mintaNotif = true;
        if (c instanceof Activity) {
            final Activity a = (Activity) c;
            new android.os.Handler(android.os.Looper.getMainLooper()).post(new Runnable() {
                @Override
                public void run() {
                    try {
                        a.requestPermissions(new String[]{"android.permission.POST_NOTIFICATIONS"}, 77);
                    } catch (Throwable t) {
                        // izin boleh gagal; notifikasi tetap dicoba
                    }
                }
            });
        }
    }

    private static PendingIntent aksi(int kode) {
        Intent i = new Intent(app, PlaybackService.class);
        i.setAction("swara.aksi." + kode);
        // PendingIntent menuju layanan: diteruskan ke onStartCommand di bawah.
        return PendingIntent.getService(app, kode, i, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
    }

    private static void tampilkan() {
        if (nm == null || sesi == null) {
            return;
        }
        Intent buka = app.getPackageManager().getLaunchIntentForPackage(app.getPackageName());
        PendingIntent pi = buka == null
                ? null
                : PendingIntent.getActivity(app, 0, buka, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);

        Notification.Builder b;
        if (Build.VERSION.SDK_INT >= 26) {
            b = new Notification.Builder(app, KANAL);
        } else {
            b = new Notification.Builder(app);
        }
        b.setSmallIcon(android.R.drawable.ic_media_play)
                .setContentTitle(judul.isEmpty() ? "Swara" : judul)
                .setContentText(artis.isEmpty() ? "Swara" : artis)
                .setOngoing(sedangMain)
                .setShowWhen(false)
                .setVisibility(Notification.VISIBILITY_PUBLIC);
        if (pi != null) {
            b.setContentIntent(pi);
        }
        b.addAction(android.R.drawable.ic_media_previous, "Sebelumnya", aksi(AKSI_PREV));
        if (sedangMain) {
            b.addAction(android.R.drawable.ic_media_pause, "Jeda", aksi(AKSI_TOGGLE));
        } else {
            b.addAction(android.R.drawable.ic_media_play, "Main", aksi(AKSI_TOGGLE));
        }
        b.addAction(android.R.drawable.ic_media_next, "Berikutnya", aksi(AKSI_NEXT));

        Notification.MediaStyle gaya = new Notification.MediaStyle()
                .setMediaSession(sesi.getSessionToken())
                .setShowActionsInCompactView(0, 1, 2);
        b.setStyle(gaya);

        nm.notify(ID, b.build());
    }

    private static void mintaFokus() {
        if (pegangFokus || app == null) {
            return;
        }
        am = (AudioManager) app.getSystemService(Context.AUDIO_SERVICE);
        if (am == null) {
            return;
        }
        if (Build.VERSION.SDK_INT >= 26) {
            AudioAttributes aa = new AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                    .build();
            fokus = new AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                    .setAudioAttributes(aa)
                    .setOnAudioFocusChangeListener(pendengar)
                    .setWillPauseWhenDucked(true)
                    .build();
            pegangFokus = am.requestAudioFocus(fokus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED;
        } else {
            pegangFokus = am.requestAudioFocus(pendengar, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN)
                    == AudioManager.AUDIOFOCUS_REQUEST_GRANTED;
        }
        if (!pegangFokus) {
            // Ditolak: jangan memutar tanpa izin, hentikan saja.
            kirim(AKSI_PAUSE);
        }
    }

    private static void lepasFokus() {
        pegangFokus = false;
        if (am == null) {
            return;
        }
        if (Build.VERSION.SDK_INT >= 26) {
            if (fokus != null) {
                am.abandonAudioFocusRequest(fokus);
            }
        } else {
            am.abandonAudioFocus(pendengar);
        }
    }

    /** Tombol di notifikasi masuk ke sini lewat intent ke {@link PlaybackService}. */
    public static void dariIntent(Intent i) {
        if (i == null || i.getAction() == null) {
            return;
        }
        String a = i.getAction();
        if (!a.startsWith("swara.aksi.")) {
            return;
        }
        try {
            kirim(Integer.parseInt(a.substring("swara.aksi.".length())));
        } catch (NumberFormatException e) {
            // abaikan
        }
    }
}
