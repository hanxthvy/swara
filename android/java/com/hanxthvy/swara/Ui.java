package com.hanxthvy.swara;

import android.os.Build;
import android.os.Bundle;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;

/**
 * NativeActivity dengan tombol back yang diserahkan ke Rust.
 *
 * Back tidak boleh langsung menutup Activity: `NativeActivity.onDestroy`
 * memanggil `dlclose` pada `libswara.so` sementara thread native masih hidup,
 * dan prosesnya menggantung (gejalanya ANR / aplikasi beku). Jadi back dikirim
 * sebagai aksi ke Rust: tutup pemutar penuh, naik satu halaman, atau pindahkan
 * aplikasi ke latar.
 *
 * Di Android 13 ke atas back lewat `OnBackInvokedDispatcher` (prediktif), jadi
 * callback-nya didaftarkan di sini; `onBackPressed` tetap ada untuk versi lama.
 */
public class Ui extends android.app.NativeActivity {
    private OnBackInvokedCallback tombol;

    @Override
    protected void onCreate(Bundle simpanan) {
        super.onCreate(simpanan);
        if (Build.VERSION.SDK_INT >= 33) {
            tombol = new OnBackInvokedCallback() {
                @Override
                public void onBackInvoked() {
                    Player.dariBelakang();
                }
            };
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT, tombol);
        }
    }

    @Override
    public void onBackPressed() {
        Player.dariBelakang();
    }
}
