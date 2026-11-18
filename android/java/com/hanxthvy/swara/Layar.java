package com.hanxthvy.swara;

import android.app.Activity;
import android.content.Context;
import android.os.Build;
import android.view.WindowInsets;

/**
 * Ukuran bagian layar yang dipakai sistem (bilah status di atas). Rust
 * memakainya sebagai jarak atas supaya judul dan tombol Swara tidak tertutup
 * jam, baterai, dan notifikasi.
 */
public class Layar {
    /** Tinggi bilah status dalam piksel fisik. */
    public static float insetAtas(Context c) {
        try {
            if (Build.VERSION.SDK_INT >= 30 && c instanceof Activity) {
                WindowInsets in = ((Activity) c).getWindow().getDecorView().getRootWindowInsets();
                if (in != null) {
                    int px = in.getInsets(WindowInsets.Type.statusBars()).top;
                    if (px > 0) {
                        return px;
                    }
                }
            }
            // Cadangan: tinggi bilah status dari sumber daya sistem.
            int id = c.getResources().getIdentifier("status_bar_height", "dimen", "android");
            if (id > 0) {
                return c.getResources().getDimensionPixelSize(id);
            }
        } catch (Throwable t) {
            // tanpa ukuran, UI memakai jarak bawaannya
        }
        return 0f;
    }
}
