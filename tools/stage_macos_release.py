"""Stage native macOS installers and signed full/light updater archives."""
import argparse
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parent.parent


def stage(kind, arch):
    if kind not in ("full", "light") or arch not in ("aarch64", "x86_64"):
        raise ValueError("Invalid macOS artifact kind or architecture")
    version = json.loads((ROOT / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    bundle = ROOT / f"desktop/src-tauri/target/{arch}-apple-darwin/release/bundle"
    dist = ROOT / "desktop/release"
    archive = bundle / "macos/Fastcloud.app.tar.gz"
    signature = archive.with_suffix(".gz.sig")
    if not archive.is_file() or not signature.read_text(encoding="utf-8").strip():
        raise ValueError("Signed macOS updater archive is missing")
    dist.mkdir(parents=True, exist_ok=True)
    suffix = "setup" if kind == "full" else "update"
    name = f"Fastcloud_{version}_macos-{arch}-{suffix}.app.tar.gz"
    shutil.copy2(archive, dist / name)
    shutil.copy2(signature, dist / (name + ".sig"))
    if kind == "full":
        installers = list((bundle / "dmg").glob("*.dmg"))
        if len(installers) != 1:
            raise ValueError("Expected exactly one macOS DMG")
        shutil.copy2(installers[0], dist / f"Fastcloud_{version}_macos-{arch}.dmg")
    print(f"Staged {kind} macOS artifacts for {arch}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("full", "light"))
    parser.add_argument("arch", choices=("aarch64", "x86_64"))
    args = parser.parse_args()
    stage(args.kind, args.arch)
