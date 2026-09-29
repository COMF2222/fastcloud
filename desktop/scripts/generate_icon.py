from pathlib import Path

from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"
root.mkdir(parents=True, exist_ok=True)

image = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((12, 12, 244, 244), radius=62, fill=(248, 103, 70, 255))
for x, height in [(75, 52), (104, 105), (133, 142), (162, 93), (191, 43)]:
    top = (256 - height) // 2
    draw.rounded_rectangle((x - 7, top, x + 7, top + height), radius=7, fill="white")

image.save(root / "icon.png")
image.resize((32, 32), Image.Resampling.LANCZOS).save(root / "32x32.png")
image.resize((128, 128), Image.Resampling.LANCZOS).save(root / "128x128.png")
image.save(root / "128x128@2x.png")
image.save(root / "icon.ico", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
image.save(root / "icon.icns", format="ICNS")
