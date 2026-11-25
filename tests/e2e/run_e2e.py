"""End-to-end tests for Unprompted.exe.

Generates AI-style images with Pillow (A1111 `parameters` tEXt, ComfyUI `prompt`/
`workflow` tEXt chunks, EXIF, XMP, trailing junk), runs the real release exe in CLI
mode, and verifies: originals untouched, outputs correct, pixels byte-identical,
metadata gone, folder input, name collisions, and error handling.

Run after `cargo build --release`:  python tests/e2e/run_e2e.py
"""

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
EXE = ROOT / "target" / "release" / "Unprompted.exe"

RESULTS = []


def check(name, cond, detail=""):
    RESULTS.append((name, bool(cond), detail))
    print(f"  {'PASS' if cond else 'FAIL'}  {name}" + (f"  ({detail})" if detail and not cond else ""))


def run_cli(*args):
    return subprocess.run(
        [str(EXE), "--cli", *[str(a) for a in args]],
        capture_output=True, text=True, timeout=120,
    )


# ---------------------------------------------------------------- image builders

def a1111_text_chunks(width, height, pixels):
    """PNG chunks: IHDR + tEXt parameters + IDAT + IEND (+ trailing junk)."""
    def chunk(ctype, data):
        c = ctype + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    raw = b"".join(b"\x00" + b"".join(bytes(p) for p in px) for px in pixels)
    idat = zlib.compress(raw)
    params = (f"Steps: 30, Sampler: Euler a, CFG scale: 7, Seed: 12345, Size: {width}x{height}, "
              "Model: dreamshaper_8, Denoising strength: 0.7").encode()
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", ihdr)
            + chunk(b"tEXt", b"parameters\x00" + params)
            + chunk(b"IDAT", idat)
            + chunk(b"IEND", b"")
            + b"AI-TRAILING-JUNK")


def make_a1111_png(path, width=64, height=48):
    pixels = [[(i * 4 % 256, j * 5 % 256, (i + j) * 3 % 256) for i in range(width)] for j in range(height)]
    path.write_bytes(a1111_text_chunks(width, height, pixels))
    return path.read_bytes()


def make_comfyui_png(path, width=48, height=40):
    img = Image.new("RGB", (width, height), (10, 120, 200))
    exif = Image.Exif(); exif[271] = "SomeCamera"          # Make
    img.save(path, "PNG", exif=exif)
    workflow = json.dumps({"1": {"class_type": "KSampler", "inputs": {"seed": 42, "steps": 20}}}).encode()
    # inject a tEXt workflow chunk before IEND
    data = path.read_bytes()
    def chunk(ctype, body):
        c = ctype + body
        return struct.pack(">I", len(body)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)
    pos = data.rindex(b"IEND") - 4  # length field of IEND
    data = data[:pos] + chunk(b"tEXt", b"prompt\x00" + workflow) + data[pos:]
    path.write_bytes(data)
    return data


def make_exif_jpeg(path):
    img = Image.new("RGB", (40, 30), (200, 60, 60))
    exif = Image.Exif()
    exif[271] = "SomeCamera"          # Make
    exif[272] = "Model X"             # Model
    exif[306] = "2024:01:01 12:00:00" # DateTime
    img.save(path, "JPEG", quality=90, exif=exif, comment=b"AI prompt steps: 20")
    return path.read_bytes()


def make_xmp_webp(path):
    img = Image.new("RGB", (40, 30), (60, 200, 90))
    xmp = b"<x:xmpmeta xmlns:x='adobe:ns:meta/'>prompt steps: 20</x:xmpmeta>"
    img.save(path, "WEBP", quality=90, xmp=xmp)
    return path.read_bytes()


# ---------------------------------------------------------------- PNG checks

def png_text_chunks(data):
    out = []
    pos = 8
    while pos + 8 <= len(data):
        length = struct.unpack(">I", data[pos:pos + 4])[0]
        ctype = data[pos + 4:pos + 8]
        if ctype == b"IEND":
            break
        if ctype in (b"tEXt", b"zTXt", b"iTXt"):
            out.append(data[pos + 8:pos + 8 + length])
        pos += 12 + length
    return out


# ---------------------------------------------------------------- scenarios

def scenario_png():
    print("Scenario: A1111 PNG")
    with tempfile.TemporaryDirectory() as d:
        src = Path(d) / "art.png"
        orig = make_a1111_png(src)
        r = run_cli(src)
        check("cli exits 0", r.returncode == 0, r.stdout + r.stderr)
        check("reports AI data removed", "AI prompt / workflow x1" in r.stdout, r.stdout)
        out = Path(d) / "art_clean.png"
        check("output exists", out.exists())
        check("original untouched", src.read_bytes() == orig)
        check("output is valid PNG", Image.open(out).format == "PNG")
        check("pixels identical", list(Image.open(out).getdata()) == list(Image.open(src).getdata()))
        check("text chunks gone", png_text_chunks(out.read_bytes()) == [])
        check("trailing junk gone", b"AI-TRAILING-JUNK" not in out.read_bytes())


def scenario_comfyui_and_folder():
    print("Scenario: ComfyUI PNG + folder input")
    with tempfile.TemporaryDirectory() as d:
        sub = Path(d) / "drop"
        sub.mkdir()
        orig = make_comfyui_png(sub / "comfy.png")
        (sub / "notes.txt").write_text("not an image")
        (sub / "auto.jpg").write_bytes(b"\xFF\xD8\xFF\xE1truncated")  # broken jpeg
        r = run_cli(sub)
        check("cli exits 1 (one bad file)", r.returncode == 1, r.stdout + r.stderr)
        check("txt skipped", "notes.txt" not in r.stdout)
        check("png cleaned", "comfy_clean.png" in r.stdout and "AI prompt / workflow" in r.stdout, r.stdout)
        check("bad jpeg reported as error", "ERROR" in r.stdout and "auto.jpg" in r.stdout, r.stdout)
        out = Path(d) / "drop" / "comfy_clean.png"
        check("output valid", out.exists() and Image.open(out).format == "PNG")
        check("original untouched", (sub / "comfy.png").read_bytes() == orig)
        check("workflow gone", not png_text_chunks(out.read_bytes()))
        check("exif make gone", b"SomeCamera" not in out.read_bytes())


def scenario_jpeg():
    print("Scenario: JPEG with EXIF + comment")
    with tempfile.TemporaryDirectory() as d:
        src = Path(d) / "photo.jpg"
        orig = make_exif_jpeg(src)
        r = run_cli(src)
        check("cli exits 0", r.returncode == 0, r.stdout + r.stderr)
        out = Path(d) / "photo_clean.jpg"
        check("output exists", out.exists())
        check("original untouched", src.read_bytes() == orig)
        data = out.read_bytes()
        check("still JPEG", data[:3] == b"\xFF\xD8\xFF")
        check("still decodes", Image.open(out).format == "JPEG")
        check("pixels identical", list(Image.open(out).getdata()) == list(Image.open(src).getdata()))
        check("exif gone", b"SomeCamera" not in data and b"Model X" not in data)
        check("comment gone", b"steps: 20" not in data)
        check("jfif kept", data[6:10] in (b"JFIF", b"Exif") or b"JFIF" in data[:30])


def scenario_webp():
    print("Scenario: WebP with XMP")
    with tempfile.TemporaryDirectory() as d:
        src = Path(d) / "gen.webp"
        orig = make_xmp_webp(src)
        r = run_cli(src)
        check("cli exits 0", r.returncode == 0, r.stdout + r.stderr)
        out = Path(d) / "gen_clean.webp"
        check("output exists", out.exists())
        check("original untouched", src.read_bytes() == orig)
        data = out.read_bytes()
        check("still RIFF/WEBP", data[:4] == b"RIFF" and data[8:12] == b"WEBP")
        check("riff size correct", struct.unpack("<I", data[4:8])[0] + 8 == len(data))
        check("still decodes", Image.open(out).format == "WEBP")
        check("pixels identical", list(Image.open(out).convert("RGB").getdata())
              == list(Image.open(src).convert("RGB").getdata()))
        check("xmp gone", b"xmpmeta" not in data)


def scenario_collision_and_nonmeta():
    print("Scenario: name collisions + clean file")
    with tempfile.TemporaryDirectory() as d:
        src = Path(d) / "art.png"
        make_a1111_png(src)
        plain = Path(d) / "plain.png"
        Image.new("RGB", (16, 16), (5, 5, 5)).save(plain)
        r = run_cli(src, plain)
        check("cli exits 0", r.returncode == 0, r.stdout + r.stderr)
        # run twice to force collisions
        r2 = run_cli(src)
        r3 = run_cli(src)
        names = sorted(p.name for p in Path(d).glob("art*.png"))
        check("collision names", names == ["art.png", "art_clean.png", "art_clean2.png", "art_clean3.png"], str(names))
        out = Path(d) / "plain_clean.png"
        check("clean file still produces output", out.exists())
        check("reports nothing to remove", "plain.png" in r.stdout and "nothing to remove" in r.stdout, r.stdout)
        check("plain output equals input", out.read_bytes() == plain.read_bytes())


def scenario_unsupported_and_missing():
    print("Scenario: unsupported / missing files")
    with tempfile.TemporaryDirectory() as d:
        txt = Path(d) / "x.txt"; txt.write_text("hi")
        gif = Path(d) / "x.gif"; Image.new("RGB", (8, 8)).save(gif)
        r = run_cli(txt, gif, Path(d) / "missing.png")
        check("cli exits 1", r.returncode == 1, r.stdout + r.stderr)
        check("gif unsupported", "not a supported image" in r.stdout, r.stdout)
        check("missing file error", "missing.png" in r.stdout, r.stdout)
        check("no outputs written", not any(Path(d).glob("*_clean*")))


def main():
    if not EXE.exists():
        print(f"exe not found: {EXE}\nRun: cargo build --release")
        return 2
    print(f"Testing {EXE} ({EXE.stat().st_size} bytes)\n")
    scenario_png()
    scenario_comfyui_and_folder()
    scenario_jpeg()
    scenario_webp()
    scenario_collision_and_nonmeta()
    scenario_unsupported_and_missing()
    failed = [n for n, ok, _ in RESULTS if not ok]
    total = len(RESULTS)
    print(f"\n{total - len(failed)}/{total} checks passed")
    if failed:
        print("FAILED: " + ", ".join(failed))
        return 1
    print("All e2e checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
