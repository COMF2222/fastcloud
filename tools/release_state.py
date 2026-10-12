"""Read publication state without mistaking an API failure for a missing release."""
import json
import os
from pathlib import Path
import subprocess


def published(result):
    if result.returncode == 0:
        draft = json.loads(result.stdout).get("isDraft")
        if not isinstance(draft, bool):
            raise ValueError("Invalid GitHub release state")
        return not draft
    if "release not found" in result.stderr.lower() or "HTTP 404" in result.stderr:
        return False
    raise RuntimeError("Could not inspect GitHub release; publication stopped")


def main():
    result = subprocess.run(["gh", "release", "view", os.environ["GITHUB_REF_NAME"],
                             "--repo", os.environ["GITHUB_REPOSITORY"], "--json", "isDraft"],
                            capture_output=True, text=True, check=False)
    value = str(published(result)).lower()
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"published={value}\n")
    print(f"Release already published: {value}")


if __name__ == "__main__":
    main()
