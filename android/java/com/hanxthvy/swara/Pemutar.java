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
public class Pemutar {
    public static final int AKSI_TOGGLE = 0;
    public static final int AKSI_NEXT = 1;
    public static final int AKSI_PREV = 2;
    public static final int AKSI_PAUSE = 3;
    public static final int AKSI_PLAY = 4;

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
    public static void perbarui(Context c, String j, String a, boolean main, long posMs, long durMs) {
        app = c.getApplicationContext();
        siapkan(c);

        boolean bergantiLagu = !j.equals(judul) || !a.equals(artis);
        boolean bergantiMain = main != sedangMain;
        judul = j;
        artis = a;
        sedangMain = main;

        MediaMetadata.Builder m = new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE, j)
                .putString(MediaMetadata.METADATA_KEY_ARTIST, a);
        if (durMs > 0) {
            m.putLong(MediaMetadata.METADATA_KEY_DURATION, durMs);
        }
        sesi.setMetadata(m.build());

        int st;
        if (main) {
            st = PlaybackState.STATE_PLAYING;
        } else if (durMs > 0) {
            st = PlaybackState.STATE_PAUSED;
        } else {
            st = PlaybackState.STATE_NONE;
        }
        sesi.setPlaybackState(new PlaybackState.Builder()
                .setActions(PlaybackState.ACTION_PLAY
                        | PlaybackState.ACTION_PAUSE
                        | PlaybackState.ACTION_PLAY_PAUSE
                        | PlaybackState.ACTION_SKIP_TO_NEXT
                        | PlaybackState.ACTION_SKIP_TO_PREVIOUS)
                .setState(st, Math.max(0, posMs), main ? 1f : 0f)
                .build());
        sesi.setActive(true);

        if (main) {
            mintaFokus();
        } else if (pegangFokus) {
            lepasFokus();
        }

        // Notifikasi hanya disegarkan saat lagu/keadaan berubah; posisi cukup
        // lewat PlaybackState supaya bilah system UI tetap halus.
        if (bergantiLagu || bergantiMain) {
            tampilkan();
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
            },
            // Thread UI Swara milik Rust tidak punya Looper; tanpa Handler
            // main thread, setCallback langsung melempar RuntimeException.
            new android.os.Handler(android.os.Looper.getMainLooper()));
        }
        mintaIzinNotifikasi(c);
    }

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
        Intent i = new Intent(app, Latar.class);
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

    /** Tombol di notifikasi masuk ke sini lewat intent ke {@link Latar}. */
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
