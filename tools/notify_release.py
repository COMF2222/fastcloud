"""Notify the approval broker only after GitHub has published the release assets."""

import json
import os
import ssl
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
    failure = "Notification delivery failed."
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=15) as response:
                result = json.load(response)
                if result.get("version") != version:
                    raise ValueError("Unexpected release acknowledgement")
            print(f"Release {version} announced to connected applications.")
            return
        except urllib.error.HTTPError as error:
            error.close()
            hint = {
                400: "Check the release version and the backend's stored version.",
                403: "The GitHub notification secret does not match the running backend's token.",
                404: "Deploy the backend with the /v1/updates/published endpoint.",
                429: "The backend rate limit was reached.",
                503: "Check that the running backend has its notification token configured and is available.",
            }.get(error.code, "Check the backend and reverse proxy.")
            failure = f"HTTP {error.code}. {hint}"
        except urllib.error.URLError as error:
            failure = ("TLS certificate verification failed. Check the server certificate."
                       if isinstance(error.reason, ssl.SSLCertVerificationError)
                       else "Network connection failed. Check server reachability from GitHub Actions.")
        except OSError:
            failure = "Network connection failed or timed out. Check server reachability from GitHub Actions."
        except ValueError:
            failure = "The server returned an invalid release acknowledgement. Check the backend and reverse proxy."
        if attempt < 2:
            time.sleep(2)
    # Do not print urllib exceptions: they may contain the private server URL.
    raise SystemExit(f"Release is published, but its server notification failed. {failure}")


if __name__ == "__main__":
    main()
