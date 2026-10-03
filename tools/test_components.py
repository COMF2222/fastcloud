import gzip
import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from prepare_components import prepare
from release_manifest import manifests
import stage_release_installer


class ComponentsTest(unittest.TestCase):
    def resources(self, root):
        files = ["worker-lite/fastcloud-clap/fastcloud-clap.exe", "model/config.json",
                 "model/tokenizer.json", "model/onnx/audio_model_quantized.onnx",
                 "model/onnx/text_model_quantized.onnx"]
        for index, name in enumerate(files):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(f"synthetic file {index}".encode())
        return files

    def test_unchanged_files_have_identical_assets_across_releases(self):
        with tempfile.TemporaryDirectory() as directory:
            root, dist = Path(directory) / "resources", Path(directory) / "release"
            self.resources(root)
            first = prepare(root, dist, "0.2.8")
            contents = {path.name: path.read_bytes() for path in dist.iterdir()}
            second = prepare(root, dist, "0.2.9")
            self.assertEqual(first["id"], second["id"])
            self.assertEqual(first["files"], second["files"])
            self.assertEqual(contents, {path.name: path.read_bytes() for path in dist.iterdir()})
            for entry in first["files"]:
                raw = gzip.decompress(contents[f"clap-{entry['sha256']}.gz"])
                self.assertEqual(hashlib.sha256(raw).hexdigest(), entry["sha256"])
                self.assertEqual(len(raw), entry["size"])

    def test_one_changed_file_creates_only_one_new_asset(self):
        with tempfile.TemporaryDirectory() as directory:
            root, dist = Path(directory) / "resources", Path(directory) / "release"
            self.resources(root)
            first = prepare(root, dist, "0.2.8")
            (root / "model/config.json").write_bytes(b"changed configuration")
            second = prepare(root, dist, "0.2.9")
            self.assertNotEqual(first["id"], second["id"])
            self.assertEqual(len(list(dist.iterdir())), len(first["files"]) + 1)
            unchanged = [a for a, b in zip(first["files"], second["files"]) if a == b]
            self.assertEqual(len(unchanged), len(first["files"]) - 1)

    def test_incomplete_model_cannot_be_published(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                prepare(Path(directory) / "missing", Path(directory) / "release", "0.2.8")

    def test_lettered_component_tag_uses_the_actual_release_name(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"GITHUB_REF_NAME": "v0.2.1a"}):
            root, dist = Path(directory) / "resources", Path(directory) / "release"
            self.resources(root)
            self.assertEqual(prepare(root, dist, "0.2.1-a", "v0.2.1a")["tag"], "v0.2.1a")

    def test_staging_keeps_both_installers_and_their_matching_signatures(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(stage_release_installer, "ROOT", Path(directory)):
            root = Path(directory)
            config = root / "desktop/src-tauri/tauri.conf.json"
            config.parent.mkdir(parents=True)
            config.write_text('{"version":"0.2.8"}')
            source = root / "desktop/src-tauri/target/release/bundle/nsis/Fastcloud_0.2.8_x64-setup.exe"
            source.parent.mkdir(parents=True)
            for kind in ("full", "light"):
                source.write_bytes(kind.encode())
                source.with_suffix(".exe.sig").write_text(f"signature-{kind}")
                stage_release_installer.stage(kind)
            for suffix, kind in (("setup", "full"), ("update", "light")):
                staged = root / f"desktop/release/Fastcloud_0.2.8_x64-{suffix}.exe"
                self.assertEqual(staged.read_bytes(), kind.encode())
                self.assertEqual(staged.with_suffix(".exe.sig").read_text(), f"signature-{kind}")

    def test_both_client_generations_get_their_own_signed_installer(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"GITHUB_REF_NAME": "v0.2.8"}):
            dist = Path(directory)
            for suffix in ("setup", "update"):
                (dist / f"Fastcloud_0.2.8_x64-{suffix}.exe").write_bytes(suffix.encode())
                (dist / f"Fastcloud_0.2.8_x64-{suffix}.exe.sig").write_text(f"signature-{suffix}")
            manifests(dist, "0.2.8")
            for filename, suffix in (("latest.json", "setup"), ("latest-light.json", "update")):
                manifest = json.loads((dist / filename).read_text())
                entry = manifest["platforms"]["windows-x86_64"]
                self.assertTrue(entry["url"].endswith(f"/v0.2.8/Fastcloud_0.2.8_x64-{suffix}.exe"))
                self.assertEqual(entry["signature"], f"signature-{suffix}")


if __name__ == "__main__":
    unittest.main()
