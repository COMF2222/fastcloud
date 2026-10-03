"""Prepare a signed Windows installer and Tauri's static update manifest."""

import hashlib
import json
from pathlib import Path
from check_release_version import release_tag

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "desktop/src-tauri/tauri.conf.json"
DIST = ROOT / "desktop/release"
RELEASE_BASE = "https://github.com/COMF2222/fastcloud/releases"


def manifests(dist: Path, version: str) -> None:
    tag = release_tag(version)
    for suffix, filename in (("setup", "latest.json"), ("update", "latest-light.json")):
        installer = dist / f"Fastcloud_{version}_x64-{suffix}.exe"
        if not installer.is_file():
            raise SystemExit(f"Missing staged installer: {installer.name}")
        signature = installer.with_suffix(".exe.sig").read_text(encoding="utf-8").strip()
        if not signature:
            raise SystemExit("Updater signature is empty")
        manifest = {"version": version, "platforms": {"windows-x86_64": {
            "signature": signature, "url": f"{RELEASE_BASE}/download/{tag}/{installer.name}",
        }}}
        (dist / filename).write_text(
            json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    lines = []
    for path in sorted(dist.iterdir()):
        if path.is_file() and path.name != "checksums.txt":
            with path.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            lines.append(f"{digest}  {path.name}")
    (dist / "checksums.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    version = json.loads(CONFIG.read_text(encoding="utf-8"))["version"]
    manifests(DIST, version)
    print(f"Prepared full and lightweight update manifests for {release_tag(version)}")


if __name__ == "__main__":
    main()
