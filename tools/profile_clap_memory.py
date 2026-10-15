"""Isolated Windows CLAP probe; never controls the running music application.

Use the same Python environment and model for --before and --after. Model
weights and samples stay local. Outputs process counters, latency and vector
differences, not user metadata. Source workers run with the production libraries.
"""
import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor


class Counters(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in (
            "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
            "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
            "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage",
            "PrivateUsage")]


class ProcessEntry(ctypes.Structure):
    _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
                ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", wintypes.LONG),
                ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260)]


def memory(pid):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
    kernel.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    # A Windows venv python.exe is a small launcher; include its real interpreter.
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == wintypes.HANDLE(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    parents = {}
    try:
        entry = ProcessEntry()
        entry.dwSize = ctypes.sizeof(entry)
        found = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        while found:
            parents[entry.th32ProcessID] = entry.th32ParentProcessID
            found = kernel.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(snapshot)
    family = {pid}
    while True:
        larger = family | {child for child, parent in parents.items() if parent in family}
        if larger == family:
            break
        family = larger
    totals = {"working_set": 0, "private": 0, "processes": len(family)}
    for child in family:
        handle = kernel.OpenProcess(0x410, False, child)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            counters = Counters()
            counters.cb = ctypes.sizeof(counters)
            if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
                raise ctypes.WinError(ctypes.get_last_error())
            totals["working_set"] += counters.WorkingSetSize
            totals["private"] += counters.PrivateUsage
        finally:
            kernel.CloseHandle(handle)
    return totals


def probe(python, worker, model):
    environment = dict(os.environ, PYTHONPATH=str(Path(__file__).resolve().parents[1] / "desktop"))
    creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
    started = time.perf_counter()
    process = subprocess.Popen([str(python), str(worker), str(model)], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                               env=environment, creationflags=creationflags)
    rows, vectors = [], []
    # Bound every read; a broken worker cannot hang the benchmark indefinitely.
    reads = ThreadPoolExecutor(max_workers=1)
    def response():
        return json.loads(reads.submit(process.stdout.readline).result(timeout=180))
    try:
        assert response()["ready"] is True
        rows.append({"stage": "ready", "seconds": round(time.perf_counter() - started, 4), **memory(process.pid)})
        with tempfile.TemporaryDirectory(prefix="veirune-clap-probe-") as folder:
            # A repeatable ten-second tone, not a user's recording.
            import array
            import math
            sample = Path(folder) / "sample.f32"
            sample.write_bytes(array.array("f", (0.1 * math.sin(i * math.tau * 440 / 48000)
                                                 for i in range(480000))).tobytes())
            requests = [
                {"kind": "text", "prompts": ["quiet acoustic music", "energetic electronic music"] * 4},
            ] + [{"kind": "audio", "path": str(sample), "rate": 48000} for _ in range(6)]
            for index, request in enumerate(requests):
                start = time.perf_counter()
                process.stdin.write(json.dumps(request) + "\n")
                process.stdin.flush()
                value = response()
                if "error" in value:
                    raise RuntimeError(value["error"])
                elapsed = time.perf_counter() - start
                vectors.extend(value["vectors"])
                rows.append({"stage": f"{index}-{request['kind']}", "seconds": round(elapsed, 4), **memory(process.pid)})
        return rows, vectors
    finally:
        process.kill()
        process.communicate(timeout=10)
        reads.shutdown(wait=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for name in ("python", "model", "before", "after"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    before, old_vectors = probe(args.python, args.before, args.model)
    after, new_vectors = probe(args.python, args.after, args.model)
    assert len(old_vectors) == len(new_vectors)
    assert all(len(a) == len(b) for a, b in zip(old_vectors, new_vectors))
    difference = max(abs(a - b) for old, new in zip(old_vectors, new_vectors) for a, b in zip(old, new))
    print(json.dumps({"before": before, "after": after, "maximum_vector_difference": difference}, indent=2))
    assert difference <= 0.000001, "Inference output changed"
