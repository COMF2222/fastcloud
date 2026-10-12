"""Package the 12 approved icon concepts for native platforms; no image recompression of wallpapers."""
from pathlib import Path
import json
import plistlib
import shutil
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
MOBILE = ROOT.parent / 'fastcloud-mobile'
WEBSITE = ROOT.parent / 'fastcloud-website'
NAMES = [('Течение', 'Flow'), ('Вектор', 'Vector'), ('Импульс', 'Pulse'),
         ('Баланс', 'Balance'), ('Веер', 'Fan'), ('Ритм', 'Rhythm'), ('Искра', 'Spark'),
         ('Орбита', 'Orbit'), ('Луна', 'Moon'), ('Контур', 'Contour'),
         ('Лента', 'Ribbon'), ('Поток', 'Stream')]

def write_png(image, path, size):
    path.parent.mkdir(parents=True, exist_ok=True)
    image.resize((size, size), Image.Resampling.LANCZOS).save(path, optimize=True)

def build():
    icons = ROOT/'assets/branding/icon-options'
    # Preserve the two earlier concepts before replacing the primary mark.
    sources = [icons/f'{i:02}.png' for i in range(1, 11)] + [icons/'11.png', icons/'12.png']
    if not sources[10].exists(): shutil.copy2(ROOT/'assets/branding/veirune-icon.png', sources[10])
    if not sources[11].exists(): shutil.copy2(ROOT/'assets/branding/veirune-icon-v2.png', sources[11])
    metadata = []
    for number, source in enumerate(sources, 1):
        image = Image.open(source).convert('RGB')
        assert image.width == image.height and image.width >= 1024
        metadata.append({'id': number, 'ru': NAMES[number-1][0], 'en': NAMES[number-1][1]})
        for public in [ROOT/'desktop/public', MOBILE/'public', WEBSITE/'public', WEBSITE/'client/public']:
            write_png(image, public/f'app-icons/{number:02}.png', 128)
        native = ROOT/f'desktop/src-tauri/resources/app-icons/{number:02}'
        write_png(image, native.with_suffix('.png'), 512)
        image.save(native.with_suffix('.ico'), sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
        res = MOBILE/'android/app/src/main/res'
        for density, size in [('mdpi',48),('hdpi',72),('xhdpi',96),('xxhdpi',144),('xxxhdpi',192)]:
            write_png(image, res/f'mipmap-{density}/veirune_{number:02}.png', size)
            write_png(image, res/f'mipmap-{density}/veirune_{number:02}_foreground.png', round(size*108/48))
        adaptive = res/f'mipmap-anydpi-v26/veirune_{number:02}.xml'
        adaptive.parent.mkdir(parents=True, exist_ok=True)
        adaptive.write_text(f'<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android"><background android:drawable="@color/icon_background"/><foreground android:drawable="@mipmap/veirune_{number:02}_foreground"/></adaptive-icon>\n')
        assets = MOBILE/f'ios/App/App/Assets.xcassets/Icon{number:02}.appiconset'
        contents = []
        for idiom, point, scale in [('iphone',20,2),('iphone',20,3),('iphone',29,2),('iphone',29,3),('iphone',40,2),('iphone',40,3),('iphone',60,2),('iphone',60,3),('ipad',20,1),('ipad',20,2),('ipad',29,1),('ipad',29,2),('ipad',40,1),('ipad',40,2),('ipad',76,1),('ipad',76,2),('ipad',83.5,2),('ios-marketing',1024,1)]:
            filename=f'{idiom}-{point}-{scale}.png'
            write_png(image, assets/filename, round(point*scale))
            contents.append({'idiom': idiom, 'size':f'{point}x{point}', 'scale':f'{scale}x', 'filename':filename})
        (assets/'Contents.json').write_text(json.dumps({'images':contents,'info':{'version':1,'author':'xcode'}},indent=2))
    for root in [ROOT/'desktop/public', MOBILE/'public', WEBSITE/'public', WEBSITE/'client/public']:
        (root/'app-icons/manifest.json').write_text(json.dumps({'default':9,'icons':metadata},ensure_ascii=False,indent=2),encoding='utf-8')
    primary = Image.open(sources[8]).convert('RGB')
    write_png(primary, ROOT/'assets/branding/veirune-icon.png', 1024)
    write_png(primary, MOBILE/'assets/branding/veirune-icon.png',1024)
    write_png(primary, ROOT/'desktop/src/assets/veirune-logo.png', 256)
    write_png(primary, MOBILE/'public/icon.png',256)
    for filename in ['veirune-icon.png','fastcloud-icon.png']:
        write_png(primary, WEBSITE/'public/assets'/filename,256)
    write_png(primary, WEBSITE/'public/favicon.png',64)
    write_png(primary, WEBSITE/'public/assets/favicon.png',64)
    write_png(primary, WEBSITE/'public/assets/veirune-favicon-moon.png',64)
    primary.save(WEBSITE/'public/favicon.ico',sizes=[(16,16),(32,32),(48,48),(64,64)])
    main_assets = MOBILE/'ios/App/App/Assets.xcassets'
    # Primary and alternative 09 have exactly the same pixels.
    for p in (main_assets/'Icon09.appiconset').iterdir():
        destination=main_assets/'AppIcon.appiconset'/p.name
        destination.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(p,destination)
    print('Packaged 12 icons; primary icon is 09 Moon.')

if __name__ == '__main__': build()
