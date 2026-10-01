from pathlib import Path

from PIL import Image

root = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"
root.mkdir(parents=True, exist_ok=True)

source = Path(__file__).resolve().parents[1] / "src" / "assets" / "fastcloud-logo.png"
image = Image.open(source).convert("RGBA")
image = image.resize((256, 256), Image.Resampling.LANCZOS)

image.save(root / "icon.png")
image.resize((32, 32), Image.Resampling.LANCZOS).save(root / "32x32.png")
image.resize((128, 128), Image.Resampling.LANCZOS).save(root / "128x128.png")
image.save(root / "128x128@2x.png")
image.save(root / "icon.ico", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
image.save(root / "icon.icns", format="ICNS")
