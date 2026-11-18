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
    /**
     * Tinggi bilah status dalam piksel fisik.
     *
     * `WindowInsets.statusBars()` di perangkat berponi bisa ikut menghitung
     * area cutout (pernah terbaca 136 px padahal bilahnya 68 px), jadi nilainya
     * dibandingkan dengan ukuran dari sumber daya sistem dan yang lebih kecil
     * yang dipakai.
     */
    public static float insetAtas(Context c) {
        int dariJendela = 0;
        int dariSumber = 0;
        try {
            if (Build.VERSION.SDK_INT >= 30 && c instanceof Activity) {
                WindowInsets in = ((Activity) c).getWindow().getDecorView().getRootWindowInsets();
                if (in != null) {
                    dariJendela = in.getInsets(WindowInsets.Type.statusBars()).top;
                }
            }
            int id = c.getResources().getIdentifier("status_bar_height", "dimen", "android");
            if (id > 0) {
                dariSumber = c.getResources().getDimensionPixelSize(id);
            }
        } catch (Throwable t) {
            // tanpa ukuran, UI memakai jarak bawaannya
        }
        if (dariJendela > 0 && dariSumber > 0) {
            return Math.min(dariJendela, dariSumber);
        }
        return dariJendela > 0 ? dariJendela : dariSumber;
    }

    /**
     * Tinggi bilah navigasi / area gestur di bawah, dalam piksel fisik. Tanpa
     * ini tombol tab paling bawah tertimpa garis gestur Android.
     */
    public static float insetBawah(Context c) {
        try {
            if (Build.VERSION.SDK_INT >= 30 && c instanceof Activity) {
                WindowInsets in = ((Activity) c).getWindow().getDecorView().getRootWindowInsets();
                if (in != null) {
                    int px = in.getInsets(WindowInsets.Type.navigationBars()).bottom;
                    if (px > 0) {
                        return px;
                    }
                }
            }
            int id = c.getResources().getIdentifier("navigation_bar_height", "dimen", "android");
            if (id > 0) {
                return c.getResources().getDimensionPixelSize(id);
            }
        } catch (Throwable t) {
            // tanpa ukuran, UI memakai jarak bawaannya
        }
        return 0f;
    }
}
