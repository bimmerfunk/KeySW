#!/usr/bin/env python3
"""Draw the KeySW icon (a keycap split blue/yellow with A and Я) and write
multi-size .ico files for the Windows app: keysw.ico (on) and keysw_off.ico.

    python3 tools/make_icons.py   # needs Pillow and the DejaVu Sans Bold font
"""
import os
from PIL import Image, ImageChops, ImageDraw, ImageFont

FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
OUT = os.path.join(os.path.dirname(__file__), "..", "crates", "keysw-win", "assets")
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def draw(size, on=True):
    s = 4 * size  # supersample, then scale down for smooth edges
    small = size <= 24
    im = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    blue = (0, 87, 183) if on else (120, 120, 120)
    yellow = (255, 215, 0) if on else (195, 195, 195)
    edge = (0, 55, 120) if on else (80, 80, 80)
    k = s / 256
    pad = 4 * k if small else 12 * k
    bottom = s - pad
    face_bottom = bottom if small else 224 * k  # no 3D edge at tray sizes
    r = (36 if small else 44) * k
    if not small:
        d.rounded_rectangle([pad, 20 * k, s - pad, bottom], radius=r, fill=edge)
    face = [pad, pad, s - pad, face_bottom]
    d.rounded_rectangle(face, radius=r, fill=blue)
    mask = Image.new("L", (s, s), 0)
    ImageDraw.Draw(mask).rounded_rectangle(face, radius=r, fill=255)
    tri = Image.new("L", (s, s), 0)
    ImageDraw.Draw(tri).polygon([(face[2], face[1]), (face[2], face[3]), (face[0], face[3])], fill=255)
    im.paste(Image.new("RGBA", (s, s), yellow), (0, 0), ImageChops.multiply(mask, tri))
    d = ImageDraw.Draw(im)
    font = ImageFont.truetype(FONT, int((128 if small else 110) * k))

    def center(txt, cx, cy, fill):
        b = d.textbbox((0, 0), txt, font=font)
        d.text((cx - (b[2] - b[0]) / 2 - b[0], cy - (b[3] - b[1]) / 2 - b[1]), txt, font=font, fill=fill)

    w, h = face[2] - face[0], face[3] - face[1]
    center("A", face[0] + w * 0.30, face[1] + h * 0.31, (255, 255, 255))
    center("Я", face[0] + w * 0.70, face[1] + h * 0.70, blue)
    return im.resize((size, size), Image.LANCZOS)


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, on in (("keysw.ico", True), ("keysw_off.ico", False)):
        imgs = [draw(n, on) for n in SIZES]
        imgs[-1].save(os.path.join(OUT, name), sizes=[(n, n) for n in SIZES], append_images=imgs[:-1])
        imgs[-1].save(os.path.join(OUT, name.replace(".ico", ".png")))
    # preview of the tray sizes
    prev = Image.new("RGBA", (200, 60), (240, 240, 240, 255))
    for i, n in enumerate((16, 24, 32)):
        prev.alpha_composite(draw(n).resize((n * 2, n * 2), Image.NEAREST), (10 + i * 64, 4))
    prev.save("/tmp/claude-0/tray_preview.png")


if __name__ == "__main__":
    main()
