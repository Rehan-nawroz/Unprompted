"""Generate the Unprompted app icon (assets/icon.ico + assets/icon.png).

Draws at 1024px with supersampling, then downsamples for crisp small sizes.
Run: python scripts/make_icon.py
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
S = 1024  # working size


def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(len(a)))


def background():
    top, bottom = (30, 41, 59, 255), (8, 12, 22, 255)
    grad = Image.new("RGBA", (S, S))
    px = grad.load()
    for y in range(S):
        c = lerp(top, bottom, y / (S - 1))
        for x in range(S):
            px[x, y] = c
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, S - 1, S - 1), radius=230, fill=255)
    out = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    out.paste(grad, (0, 0), mask)
    return out


def glow(size_box, color, blur):
    layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(layer).ellipse(size_box, fill=color)
    return layer.filter(ImageFilter.GaussianBlur(blur))


def draw_icon():
    img = background()

    # soft emerald glow behind the frame
    img = Image.alpha_composite(img, glow((220, 220, 800, 800), (16, 185, 129, 90), 90))

    d = ImageDraw.Draw(img)
    emerald = (16, 185, 129, 255)
    emerald_hi = (52, 211, 153, 255)

    # image frame
    frame = (190, 250, 834, 774)
    d.rounded_rectangle(frame, radius=64, fill=(15, 23, 42, 255), outline=emerald, width=44)

    # sun
    d.ellipse((560, 340, 680, 460), fill=emerald_hi)

    # mountains (two peaks, clipped to the frame interior)
    inner = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    idr = ImageDraw.Draw(inner)
    idr.polygon([(190, 774), (190, 600), (370, 430), (500, 560), (600, 470), (834, 700), (834, 774)],
                fill=emerald)
    idr.polygon([(190, 774), (190, 690), (300, 610), (420, 700), (420, 774)], fill=emerald_hi)
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (frame[0] + 22, frame[1] + 22, frame[2] - 22, frame[3] - 22), radius=44, fill=255)
    img.paste(inner, (0, 0), Image.composite(inner, Image.new("RGBA", (S, S), (0, 0, 0, 0)), mask))

    # "clean" badge: white disc with an emerald check, bottom-right
    img = Image.alpha_composite(img, glow((610, 610, 930, 930), (0, 0, 0, 160), 26))
    d = ImageDraw.Draw(img)
    d.ellipse((600, 600, 920, 920), fill=(248, 250, 252, 255))
    d.ellipse((616, 616, 904, 904), outline=emerald, width=0)
    d.line([(676, 764), (734, 822), (844, 690)], fill=(5, 150, 105, 255), width=44, joint="curve")
    return img


def main():
    ASSETS.mkdir(exist_ok=True)
    big = draw_icon()
    png256 = big.resize((256, 256), Image.LANCZOS)
    png256.save(ASSETS / "icon.png")
    sizes = [16, 24, 32, 48, 64, 128, 256]
    ico_src = big.resize((256, 256), Image.LANCZOS)
    ico_src.save(ASSETS / "icon.ico", sizes=[(s, s) for s in sizes])
    print("wrote", ASSETS / "icon.ico", "and", ASSETS / "icon.png")


if __name__ == "__main__":
    main()
