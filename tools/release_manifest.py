"""Prepare a signed Windows installer and Tauri's static update manifest."""

import hashlib
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "desktop/src-tauri/tauri.conf.json"
INSTALLERS = ROOT / "desktop/src-tauri/target/release/bundle/nsis"
DIST = ROOT / "desktop/release"
RELEASE_BASE = "https://github.com/COMF2222/fastcloud/releases"


def main() -> None:
    version = json.loads(CONFIG.read_text(encoding="utf-8"))["version"]
    installers = list(INSTALLERS.glob(f"Fastcloud_{version}_x64-setup.exe"))
    if len(installers) != 1:
        raise SystemExit(f"Expected one NSIS installer for {version}; found {len(installers)}")
    installer = installers[0]
    signature_file = installer.with_suffix(installer.suffix + ".sig")
    signature = signature_file.read_text(encoding="utf-8").strip()
    if not signature:
        raise SystemExit("Updater signature is empty")
    DIST.mkdir(parents=True, exist_ok=True)
    for source in (installer, signature_file):
        shutil.copy2(source, DIST / source.name)
    manifest = {
        "version": version,
        "platforms": {
            "windows-x86_64": {
                "signature": signature,
                "url": f"{RELEASE_BASE}/download/v{version}/{installer.name}",
            }
        },
    }
    (DIST / "latest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    lines = []
    for path in sorted(DIST.iterdir()):
        if path.is_file() and path.name != "checksums.txt":
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            lines.append(f"{digest}  {path.name}")
    (DIST / "checksums.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Prepared {installer.name}, signature, and latest.json for v{version}")


if __name__ == "__main__":
    main()
