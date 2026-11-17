package com.hanxthvy.swara;

import android.app.Activity;
import android.app.Application;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import android.os.IBinder;

/**
 * Foreground service: tanpa ini Android membunuh proses Swara beberapa detik
 * setelah layar mati, dan musik berhenti sendiri.
 */
public class Latar extends Service {
    private static final String KANAL = "swara_putar";
    private static final int ID = 1001;

    public static void mulai(Context ctx) {
        Intent i = new Intent(ctx, Latar.class);
        if (Build.VERSION.SDK_INT >= 26) {
            ctx.startForegroundService(i);
        } else {
            ctx.startService(i);
        }
    }

    private static boolean dipantau = false;
    private static volatile boolean penjagaJalan = false;

    /**
     * Penjaga tugas di thread sendiri (bukan main looper): kalau kartu
     * aplikasi tidak ada lagi di daftar recent, matikan proses.
     *
     * Ini perlu karena jalur resmi tidak bisa dipakai: saat tugas dihapus,
     * Android menghancurkan Activity dan `NativeActivity.onDestroy` memanggil
     * dlclose pada libswara.so sementara thread native kita masih berjalan.
     * Proses menggantung di dlclose itu, `onTaskRemoved` tidak pernah sempat
     * jalan, dan membuka aplikasi lagi berakhir mentok di splash.
     */
    private void pasangPenjaga() {
        if (penjagaJalan) {
            return;
        }
        penjagaJalan = true;
        Thread t = new Thread(new Runnable() {
            @Override
            public void run() {
                boolean pernahAda = false;
                while (true) {
                    try {
                        Thread.sleep(1000);
                    } catch (InterruptedException e) {
                        return;
                    }
                    boolean ada = false;
                    try {
                        android.app.ActivityManager am =
                                (android.app.ActivityManager) getSystemService(Context.ACTIVITY_SERVICE);
                        if (am != null) {
                            ada = !am.getAppTasks().isEmpty();
                        }
                    } catch (Throwable e) {
                        // anggap masih ada; jangan mematikan karena galat baca
                        ada = true;
                    }
                    if (ada) {
                        pernahAda = true;
                    } else if (pernahAda) {
                        android.util.Log.i("Swara", "tugas dihapus dari recent: proses dihentikan");
                        stopForeground(true);
                        stopSelf();
                        android.os.Process.killProcess(android.os.Process.myPid());
                        return;
                    }
                }
            }
        }, "penjaga-tugas");
        t.setDaemon(true);
        t.start();
    }

    /**
     * Pantau siklus Activity. Layanan foreground menjaga proses tetap hidup,
     * jadi saat Activity dihancurkan (kartu di-swipe dari daftar recent,
     * activity dimatikan sistem) prosesnya harus dimatikan dari sini juga.
     */
    private void pantauActivity() {
        if (dipantau) {
            return;
        }
        dipantau = true;
        getApplication().registerActivityLifecycleCallbacks(new Application.ActivityLifecycleCallbacks() {
            @Override
            public void onActivityCreated(Activity a, Bundle b) {
            }

            @Override
            public void onActivityStarted(Activity a) {
            }

            @Override
            public void onActivityResumed(Activity a) {
            }

            @Override
            public void onActivityPaused(Activity a) {
            }

            @Override
            public void onActivityStopped(Activity a) {
            }

            @Override
            public void onActivitySaveInstanceState(Activity a, Bundle b) {
            }

            @Override
            public void onActivityDestroyed(Activity a) {
                // Putaran layar dan perubahan konfigurasi juga menghancurkan
                // Activity; itu bukan alasan untuk mematikan pemutaran.
                if (a.isChangingConfigurations()) {
                    return;
                }
                android.util.Log.i("Swara", "activity dihancurkan: proses dihentikan");
                android.os.Process.killProcess(android.os.Process.myPid());
            }
        });
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        // Tombol notifikasi dikirim sebagai intent ke layanan ini.
        Pemutar.dariIntent(intent);
        pantauActivity();
        pasangPenjaga();
        if (Build.VERSION.SDK_INT >= 26) {
            NotificationManager nm = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
            NotificationChannel c = new NotificationChannel(KANAL, "Pemutaran", NotificationManager.IMPORTANCE_LOW);
            c.setShowBadge(false);
            nm.createNotificationChannel(c);
        }
        Notification.Builder b;
        if (Build.VERSION.SDK_INT >= 26) {
            b = new Notification.Builder(this, KANAL);
        } else {
            b = new Notification.Builder(this);
        }
        b.setContentTitle("Swara")
                .setContentText("Memutar musik")
                .setSmallIcon(android.R.drawable.ic_media_play)
                .setOngoing(true);
        startForeground(ID, b.build());
        // JANGAN START_STICKY. Layanan yang dihidupkan ulang tanpa activity
        // membuat proses tetap hidup setelah aplikasi di-swipe dari daftar
        // recent -- musik jalan terus padahal aplikasinya "sudah ditutup".
        return START_NOT_STICKY;
    }

    /**
     * Swipe dari daftar recent = tutup total: hentikan layanan lalu matikan
     * proses. Tanpa ini proses hantu tetap ada, dan membuka aplikasi lagi
     * berakhir mentok di splash karena android_main dipanggil dua kali di
     * proses yang sama.
     */
    @Override
    public void onTaskRemoved(Intent rootIntent) {
        super.onTaskRemoved(rootIntent);
        android.util.Log.i("Swara", "kartu recent di-swipe: layanan dan proses dihentikan");
        stopForeground(true);
        stopSelf();
        android.os.Process.killProcess(android.os.Process.myPid());
    }

    /**
     * Dipanggil Rust ketika Activity sudah dihancurkan (kartu di-swipe dari
     * daftar recent, tombol back, atau sistem menutup tugas). Layanan dan
     * proses dihentikan supaya tidak ada proses hantu yang memutar musik
     * tanpa jendela.
     */
    public static void berhenti(Context ctx) {
        try {
            ctx.stopService(new Intent(ctx, Latar.class));
        } catch (Throwable t) {
            // abaikan
        }
        android.os.Process.killProcess(android.os.Process.myPid());
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
