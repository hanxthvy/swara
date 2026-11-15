package com.hanxthvy.swara;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.graphics.Color;
import android.os.Bundle;
import android.webkit.CookieManager;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;

/**
 * Halaman login memakai WebView sistem. Setelah pengguna masuk, cookie
 * music.youtube.com ditulis ke berkas privat aplikasi; Rust membacanya lalu
 * menyusun sesi sendiri. Tidak ada kata sandi yang lewat tangan kita.
 */
public class Masuk extends Activity {
    private WebView web;

    public static void buka(Context ctx) {
        Intent i = new Intent(ctx, Masuk.class);
        i.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        ctx.startActivity(i);
    }

    @Override
    protected void onCreate(Bundle simpan) {
        super.onCreate(simpan);
        web = new WebView(this);
        web.setBackgroundColor(Color.rgb(11, 11, 12));
        web.getSettings().setJavaScriptEnabled(true);
        web.getSettings().setDomStorageEnabled(true);
        web.getSettings().setDatabaseEnabled(true);
        web.getSettings().setLoadWithOverviewMode(true);
        web.getSettings().setUseWideViewPort(true);
        CookieManager.getInstance().setAcceptCookie(true);
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, true);
        web.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageFinished(WebView v, String url) {
                catat();
            }
        });
        web.loadUrl("https://accounts.google.com/ServiceLogin?service=youtube"
                + "&continue=https%3A%2F%2Fmusic.youtube.com");
        setContentView(web);
    }

    private void catat() {
        String c = "";
        try {
            String a = CookieManager.getInstance().getCookie("https://music.youtube.com");
            String b = CookieManager.getInstance().getCookie("https://www.youtube.com");
            if (b != null && !b.isEmpty()) {
                c = b;
            }
            if (a != null && !a.isEmpty()) {
                c = c.isEmpty() ? a : c + "; " + a;
            }
            File f = new File(getFilesDir(), "cookies.txt");
            FileOutputStream o = new FileOutputStream(f);
            o.write(c.getBytes("UTF-8"));
            o.close();
        } catch (Exception e) {
            // biarkan: kalau gagal, Rust menganggap belum ada sesi
        }
    }

    /** Dipanggil dari Rust lewat JNI. */
    public static String cookie(Context ctx) {
        try {
            File f = new File(ctx.getFilesDir(), "cookies.txt");
            if (!f.exists()) {
                return "";
            }
            byte[] b = new byte[(int) f.length()];
            FileInputStream i = new FileInputStream(f);
            i.read(b);
            i.close();
            return new String(b, "UTF-8");
        } catch (Exception e) {
            return "";
        }
    }

    @Override
    protected void onPause() {
        super.onPause();
        catat();
    }
}
