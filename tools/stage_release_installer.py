"""Keep the full and lightweight signed NSIS artifacts from two bundle passes."""
from release_brand import asset_prefix
import argparse
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def stage(kind):
    config = json.loads((ROOT / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    version = config["version"]
    product = config.get("productName", "Fastcloud")
    source = ROOT / f"desktop/src-tauri/target/release/bundle/nsis/{product}_{version}_x64-setup.exe"
    suffix = "setup" if kind == "full" else "update"
    name = f"{asset_prefix(version)}_{version}_x64-{suffix}.exe"
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
