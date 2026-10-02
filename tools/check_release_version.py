"""Reject a tag that would publish installers with a different embedded version."""

import json
import os
import re
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def release_tag(version):
    canonical = "v" + version
    compact = re.sub(r"-([a-z])$", r"\1", canonical)
    tag = os.environ.get("GITHUB_REF_NAME", compact)
    if tag not in (canonical, compact):
        raise SystemExit(f"Release tag {tag} does not match application version {version}.")
    return tag


def main():
    def read_json(path):
        return json.loads((ROOT / path).read_text(encoding="utf-8"))

    def read_toml(path):
        return tomllib.loads((ROOT / path).read_text(encoding="utf-8"))

    version = read_json("desktop/src-tauri/tauri.conf.json")["version"]
    tag = release_tag(version)
    versions = {
        "Cargo.toml": read_toml("Cargo.toml")["package"]["version"],
        "desktop/src-tauri/Cargo.toml": read_toml("desktop/src-tauri/Cargo.toml")["package"]["version"],
        "desktop/package.json": read_json("desktop/package.json")["version"],
        "desktop/package-lock.json": read_json("desktop/package-lock.json")["version"],
        "desktop/package-lock.json root package": read_json("desktop/package-lock.json")["packages"][""]["version"],
    }
    for path, name in (("Cargo.lock", "fastcloud"), ("desktop/src-tauri/Cargo.lock", "fastcloud-desktop")):
        versions[path] = next(package["version"] for package in read_toml(path)["package"] if package["name"] == name)
    for path, actual in versions.items():
        if actual != version:
            raise SystemExit(f"{path} has version {actual}; expected {version}.")
    print(f"Release {tag} ({version}): tag, manifests and lockfiles agree.")


if __name__ == "__main__":
    main()
