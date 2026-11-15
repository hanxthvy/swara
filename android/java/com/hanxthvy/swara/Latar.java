package com.hanxthvy.swara;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
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

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
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
        return START_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
