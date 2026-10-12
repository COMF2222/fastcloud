"""Smoke-test a native cached worker with a bounded timeout on every platform."""
import json
from pathlib import Path
import subprocess
import sys


def verify(worker, model):
    result = subprocess.run([str(worker), str(model)],
                            input='{"kind":"text","prompts":["happy music"]}\n',
                            text=True, capture_output=True, timeout=180, check=True)
    rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
    vectors = rows[-1].get('vectors', []) if rows else []
    if not rows or rows[0].get('ready') is not True or len(vectors) != 1 or not isinstance(vectors[0], list) or len(vectors[0]) < 256:
        raise ValueError('Native CLAP worker smoke test failed')


if __name__ == '__main__':
    verify(Path(sys.argv[1]), Path(sys.argv[2]))
