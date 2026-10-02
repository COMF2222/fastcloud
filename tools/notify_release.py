"""Notify the approval broker only after GitHub has published the release assets."""

import json
import os
import time
import urllib.error
import urllib.request
from pathlib import Path


def main():
    token = os.environ.get("FASTCLOUD_RELEASE_NOTIFY_TOKEN", "")
    if not token:
        print("::warning::Release published; push notifications skipped because FASTCLOUD_RELEASE_NOTIFY_TOKEN is missing.")
        return
    origin = os.environ["VITE_FASTCLOUD_SERVER_URL"].rstrip("/")
    config = Path(__file__).resolve().parent.parent / "desktop/src-tauri/tauri.conf.json"
    version = json.loads(config.read_text(encoding="utf-8"))["version"]
    body = json.dumps({"version": version}).encode()
    request = urllib.request.Request(origin + "/v1/updates/published", data=body, headers={
        "Authorization": "Bearer " + token, "Content-Type": "application/json",
    })
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=15) as response:
                result = json.load(response)
                if result.get("version") != version:
                    raise ValueError("Unexpected release acknowledgement")
            print(f"Release {version} announced to connected applications.")
            return
        except (urllib.error.URLError, OSError, ValueError):
            if attempt < 2:
                time.sleep(2)
    # Do not print urllib exceptions: they may contain the private server URL.
    raise SystemExit("Release is published, but its server notification failed. Check backend deployment and the shared notification secret.")


if __name__ == "__main__":
    main()
