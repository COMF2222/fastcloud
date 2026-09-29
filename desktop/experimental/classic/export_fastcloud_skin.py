"""Export the built-in egui Fastcloud skin to webview bitmap sheets.

This follows src/skin/classic.rs. The source archive is MIT licensed and its
license is shipped beside the generated sheets.
"""

from io import BytesIO
from pathlib import Path
from zipfile import ZipFile

from PIL import Image


ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / "desktop" / "public" / "skins" / "fastcloud"
DEST.mkdir(parents=True, exist_ok=True)

with ZipFile(ROOT / "assets" / "skins" / "fastpotify-base.wsz") as archive:
    sheets = {}
    for name in archive.namelist():
        leaf = Path(name).name.lower()
        if leaf.endswith(".bmp"):
            bitmap = Image.open(BytesIO(archive.read(name))).convert("RGB")
            pixels = bitmap.load()
            for y in range(bitmap.height):
                for x in range(bitmap.width):
                    r, g, b = pixels[x, y]
                    if g > r and g > b:
                        pixels[x, y] = (g, min(255, r + (g - r) // 3), r // 3)
            sheets[leaf] = bitmap

font = sheets["text.bmp"]
bar = sheets["titlebar.bmp"]
rows = [
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ\"@",
    "0123456789….:()-'!_+\\/[]^&%,=$#",
    "ÅÖÄ?*",
]
background = font.getpixel((font.width - 1, 0))
for y, active, shade in [
    (0, True, False), (15, False, False),
    (29, True, True), (42, False, True),
    (57, True, False), (72, False, False),
]:
    x = 47 if shade else 137
    width = 50 if shade else 56
    for py in range(y + 3, y + 11):
        for px in range(x, x + width):
            bar.putpixel((px, py), (29, 33, 39))
    ink = (242, 244, 246) if active else (110, 119, 132)
    start = x if shade else 27 + (275 - 44) // 2
    for index, letter in enumerate("FASTCLOUD"):
        row, col = next((row, chars.index(letter)) for row, chars in enumerate(rows) if letter in chars)
        for gy in range(6):
            for gx in range(5):
                if font.getpixel((col * 5 + gx, row * 6 + gy)) != background:
                    bar.putpixel((start + index * 5 + gx, y + 4 + gy), ink)

for name, image in sheets.items():
    image.save(DEST / name)
(DEST / "LICENSE.txt").write_bytes((ROOT / "assets" / "skins" / "LICENSE-Fastpotify.txt").read_bytes())
