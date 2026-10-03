"""Keep the full and lightweight signed NSIS artifacts from two bundle passes."""
import argparse
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def stage(kind):
    version = json.loads((ROOT / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    source = ROOT / f"desktop/src-tauri/target/release/bundle/nsis/Fastcloud_{version}_x64-setup.exe"
    name = source.name if kind == "full" else f"Fastcloud_{version}_x64-update.exe"
    destination = ROOT / "desktop/release" / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not source.is_file() or not source.with_suffix(".exe.sig").read_text(encoding="utf-8").strip():
        raise SystemExit("Signed installer is missing")
    shutil.copy2(source, destination)
    shutil.copy2(source.with_suffix(".exe.sig"), destination.with_suffix(".exe.sig"))
    print(f"Staged {name}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("full", "light"))
    stage(parser.parse_args().kind)
