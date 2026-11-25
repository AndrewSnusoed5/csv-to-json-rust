"""Compare the release binary with the pinned Node.js program."""

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / "reference"
DATA = REFERENCE / "data"
DATASETS = {
    "energy.csv": (
        "https://raw.githubusercontent.com/owid/energy-data/"
        "f8de88a4f6f9cab44f1cf606047cbd6c4414042c/owid-energy-data.csv",
        "266f2e2baad7975351bc9bb4aa061d22b1da9fe4c47d51d2ac6071e01e171f76",
    ),
    "taxi-zones.csv": (
        "https://d37ci6vzurychx.cloudfront.net/misc/taxi_zone_lookup.csv",
        "1a99e105092230f8620f301edcca7f80d3080642ff404d28ed957d3fa222c8ed",
    ),
}
SMALL_CASES = {
    "numeric-headers": (b"5|1|name\nx|y|z\n", ("|", "LF", "header", "skip")),
    "duplicate-headers": (
        b"5,name,tag,name,1\nx,first,extra,last,y\n", (",", "LF", "header", "skip"),
    ),
    "bom-only": (b"\xef\xbb\xbf", (",", "LF", "header", "skip")),
    "multiline": (
        '\ufeffid\tname\tvalue\r\n1\t"é🙂\r\nx"\t"q""r"\r\n2\t"value"\t""\r\n'.encode(),
        ("\t", "CRLF", "header", "skip"),
    ),
    "skip-blank-records": (b'""\n\n"not empty"\n', (",", "LF", "arrays", "skip")),
    "keep-blank-records": (b'a\n\n""\n"b\n\nc"\n', (",", "LF", "arrays", "keep")),
    "terminal-separator": (b"a;b;\nc;d;\n", (";", "LF", "arrays", "keep")),
}

if os.name == "nt":
    class ProcessMemory(ctypes.Structure):
        _fields_ = [
            ("cb", ctypes.c_ulong),
            ("page_faults", ctypes.c_ulong),
            ("peak_working_set", ctypes.c_size_t),
            ("working_set", ctypes.c_size_t),
            ("peak_paged_pool", ctypes.c_size_t),
            ("paged_pool", ctypes.c_size_t),
            ("peak_nonpaged_pool", ctypes.c_size_t),
            ("nonpaged_pool", ctypes.c_size_t),
            ("pagefile", ctypes.c_size_t),
            ("peak_pagefile", ctypes.c_size_t),
        ]

    memory_info = ctypes.WinDLL("psapi").GetProcessMemoryInfo
    memory_info.argtypes = (ctypes.c_void_p, ctypes.POINTER(ProcessMemory), ctypes.c_ulong)
    memory_info.restype = ctypes.c_int


def peak_rss_mib(process):
    if os.name != "nt":
        return None
    counters = ProcessMemory()
    counters.cb = ctypes.sizeof(counters)
    if memory_info(int(process._handle), ctypes.byref(counters), counters.cb):
        return counters.peak_working_set / 2**20
    return None


def dataset(filename):
    DATA.mkdir(exist_ok=True)
    url, checksum = DATASETS[filename]
    path = DATA / filename
    if path.is_file():
        content = path.read_bytes()
    else:
        with urlopen(url, timeout=60) as response:
            content = response.read()
    if hashlib.sha256(content).hexdigest() != checksum:
        raise ValueError(f"Unexpected SHA-256 for {filename}")
    if not path.is_file():
        path.write_bytes(content)
    return path


def run(command, directory):
    output = directory / "rows.json"
    output.unlink(missing_ok=True)
    start = time.perf_counter_ns()
    with subprocess.Popen(command, cwd=directory, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as process:
        try:
            stdout, stderr = process.communicate(timeout=60)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
            raise
        elapsed = (time.perf_counter_ns() - start) / 1_000_000
        if process.returncode:
            raise RuntimeError(stderr.decode(errors="replace"))
        rss = peak_rss_mib(process)
    return stdout.replace(b"\r\n", b"\n"), output.read_bytes(), elapsed, rss


def stats(samples):
    wall, rss = zip(*samples)
    memory = [value for value in rss if value is not None]
    return {"wall_ms": statistics.median(wall), "peak_rss_mib": statistics.median(memory) if memory else None}


def main():
    suffix = ".exe" if os.name == "nt" else ""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target" / "release" / f"csv-to-json{suffix}")
    parser.add_argument("--warmup", type=int, default=2)
    parser.add_argument("--iterations", type=int, default=7)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.warmup < 0 or args.iterations < 1:
        parser.error("Expected non-negative warm-ups and at least one measured iteration")
    reports = {}
    with tempfile.TemporaryDirectory(prefix="csv-to-json-") as temp:
        workdir = Path(temp)
        cases = [
            ("nyc-taxi-zones", dataset("taxi-zones.csv"), (",", "CRLF", "header", "skip")),
            ("owid-energy", dataset("energy.csv"), (",", "LF", "arrays", "skip")),
        ]
        for name, (content, flags) in SMALL_CASES.items():
            path = workdir / f"{name}.csv"
            path.write_bytes(content)
            cases.append((name, path, flags))

        for name, path, flags in cases:
            node_samples, rust_samples = [], []
            arguments = [str(path), *flags]
            node_command = ["node", str(REFERENCE / "csv-import.mjs"), *arguments]
            rust_command = [str(args.binary.resolve()), *arguments]
            for iteration in range(args.warmup + args.iterations):
                if iteration % 2 == 0:
                    node = run(node_command, workdir)
                    rust = run(rust_command, workdir)
                else:
                    rust = run(rust_command, workdir)
                    node = run(node_command, workdir)
                if node[:2] != rust[:2]:
                    raise AssertionError(f"{name}, pair {iteration + 1}: stdout or JSON differs")
                if iteration >= args.warmup:
                    node_samples.append(node[2:])
                    rust_samples.append(rust[2:])
            baseline, native = stats(node_samples), stats(rust_samples)
            speedup = baseline["wall_ms"] / native["wall_ms"]
            reports[name] = {
                "node": baseline, "rust": native, "speedup": speedup,
                "result_bytes": len(rust[1]), "rows": int(rust[0]),
            }
            print(f"{name}: equal, {baseline['wall_ms']:.2f} ms -> {native['wall_ms']:.2f} ms ({speedup:.2f}x)")
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps({
            "warmup_pairs": args.warmup, "measured_pairs": args.iterations, "cases": reports,
        }, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
