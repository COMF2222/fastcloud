"""Publish CLAP files by content hash; embed their manifest in the signed app."""

import gzip
import hashlib
import json
import shutil
from pathlib import Path
from check_release_version import release_tag

ROOT = Path(__file__).resolve().parent.parent
RESOURCE_ROOT = ROOT / "desktop/src-tauri/resources/clap"
MANIFEST = ROOT / "desktop/src-tauri/resources/component-manifest.json"
DIST = ROOT / "desktop/release"


def resource_files(root):
    files = list((root / "worker-lite/fastcloud-clap").rglob("*"))
    files += list((root / "model").glob("*.json"))
    files += list((root / "model").glob("*.txt"))
    files += [root / "model/onnx" / name for name in
              ("audio_model_quantized.onnx", "text_model_quantized.onnx")]
    files += list((root / "licenses").glob("*.txt"))
    files = sorted({path for path in files if path.is_file()})
    worker = root / "worker-lite/fastcloud-clap/fastcloud-clap.exe"
    if not worker.is_file(): worker = root / "worker-lite/fastcloud-clap/fastcloud-clap"
    required = [worker,
                root / "model/config.json", root / "model/tokenizer.json",
                root / "model/onnx/audio_model_quantized.onnx",
                root / "model/onnx/text_model_quantized.onnx"]
    if any(not path.is_file() for path in required):
        raise ValueError("Prepare the CLAP worker and model before publishing components")
    if any(path.is_symlink() for path in files):
        raise ValueError("CLAP resources must be regular files")
    return files


def prepare(root, dist, version, tag=None):
    dist.mkdir(parents=True, exist_ok=True)
    entries = []
    for path in resource_files(root):
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        entries.append({"path": path.relative_to(root).as_posix(),
                        "sha256": digest, "size": path.stat().st_size})
        asset = dist / f"clap-{digest}.gz"
        if not asset.exists():
            with path.open("rb") as source, asset.open("wb") as destination:
                # Stable gzip headers keep identical resources identical between releases.
                with gzip.GzipFile(filename="", mode="wb", fileobj=destination, mtime=0) as compressed:
                    shutil.copyfileobj(source, compressed)
    entries.sort(key=lambda entry: entry["path"])
    canonical = json.dumps(entries, ensure_ascii=False, separators=(",", ":")).encode()
    return {"schema": 1, "version": version, "tag": tag or "v" + version,
            "id": hashlib.sha256(canonical).hexdigest(), "files": entries}


def main():
    version = json.loads((ROOT / "desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["version"]
    manifest = prepare(RESOURCE_ROOT, DIST, version, release_tag(version))
    MANIFEST.write_text(json.dumps(manifest, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(f"Prepared {len(manifest['files'])} reusable CLAP files ({manifest['id'][:12]})")


if __name__ == "__main__":
    main()
