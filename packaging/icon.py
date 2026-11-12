#!/usr/bin/env python3
"""Bikin ikon Swara (PNG, berbagai ukuran) tanpa dependensi font."""
import math
import os
from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

# latar: kuadrat membulat dengan gradien diagonal ungu -> lavender
bg = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(bg)
for y in range(S):
    t = y / (S - 1)
    r = int(96 + (182 - 96) * t)
    g = int(74 + (156 - 74) * t)
    b = int(214 + (255 - 214) * t)
    d.line([(0, y), (S, y)], fill=(r, g, b, 255))
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=int(S * 0.24), fill=255)
img.paste(bg, (0, 0), mask)

dr = ImageDraw.Draw(img)
white = (255, 255, 255, 255)
soft = (255, 255, 255, 150)

# nada: dua kepala not + tangkai + balok
cx, cy = S * 0.5, S * 0.56
head_w, head_h = S * 0.132, S * 0.108
for dx in (-S * 0.125, S * 0.125):
    dr.ellipse(
        [cx + dx - head_w, cy - head_h / 2, cx + dx + head_w, cy + head_h / 2],
        fill=white,
    )
stem_w = S * 0.032
for dx in (-S * 0.125 + head_w * 0.72, S * 0.125 + head_w * 0.72):
    dr.rounded_rectangle(
        [cx + dx - stem_w / 2, cy - S * 0.30, cx + dx + stem_w / 2, cy],
        radius=stem_w / 2,
        fill=white,
    )
dr.polygon(
    [
        (cx - S * 0.125 + head_w * 0.72 - stem_w / 2, cy - S * 0.30),
        (cx + S * 0.125 + head_w * 0.72 + stem_w / 2, cy - S * 0.26),
        (cx + S * 0.125 + head_w * 0.72 + stem_w / 2, cy - S * 0.17),
        (cx - S * 0.125 + head_w * 0.72 - stem_w / 2, cy - S * 0.21),
    ],
    fill=white,
)

# gelombang suara halus di sisi kanan (isyarat "swara")
for i, rad in enumerate((0.40, 0.47)):
    rr = S * rad
    dr.arc(
        [cx - rr, cy - rr, cx + rr, cy + rr],
        start=-38,
        end=38,
        fill=soft,
        width=int(S * 0.018),
    )

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "swara.png")
img.resize((512, 512), Image.LANCZOS).save(out)
print(out)
