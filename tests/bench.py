#!/usr/bin/env python3
"""Small HTTP/1.1 baseline (includes Python client and new TCP connection costs).

Build the desired host profile and the sample before running:
  python3 tests/bench.py --binary target/debug/yamanami
  cargo build --release --locked
  python3 tests/bench.py --binary target/release/yamanami
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import tempfile
import time

import e2e


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/yamanami")
    parser.add_argument("--requests", type=int, default=100)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    if args.requests < 1 or not 1 <= args.workers <= 128:
        parser.error("requests must be positive and workers must be 1..128")
    e2e.BINARY = (e2e.ROOT / args.binary).resolve()
    wasm = e2e.GUEST.read_bytes()
    with tempfile.TemporaryDirectory(prefix="yamanami-bench-") as directory:
        server = e2e.Server(directory, [
            "--concurrency", str(args.workers),
            "--app-concurrency", str(args.workers),
        ]).start()
        try:
            started = time.perf_counter()
            status, _, body = server.deploy("hello", "/hello", wasm)
            deploy_seconds = time.perf_counter() - started
            if status != 201:
                raise RuntimeError(body.decode())
            for _ in range(5):
                if e2e.request(server.public, "GET", "/hello")[0] != 200:
                    raise RuntimeError("warmup failed")

            def fetch(_):
                started = time.perf_counter()
                status, _, body = e2e.request(server.public, "GET", "/hello")
                return time.perf_counter() - started, status, len(body)

            started = time.perf_counter()
            with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
                results = list(pool.map(fetch, range(args.requests)))
            elapsed = time.perf_counter() - started
            latencies = sorted(r[0] * 1000 for r in results)
            codes = {}
            for _, code, _ in results:
                codes[str(code)] = codes.get(str(code), 0) + 1
            rss = {}
            proc_status = Path(f"/proc/{server.proc.pid}/status")
            if proc_status.exists():
                for line in proc_status.read_text().splitlines():
                    if line.startswith(("VmRSS:", "VmHWM:")):
                        key, value = line.split(":", 1)
                        rss[key + "_kib"] = int(value.split()[0])
            cpu = "unknown"
            cpuinfo = Path("/proc/cpuinfo")
            if cpuinfo.exists():
                cpu = next((line.split(":", 1)[1].strip()
                            for line in cpuinfo.read_text().splitlines()
                            if line.startswith("model name")), cpu)
            print(json.dumps({
                "scope": "local HTTP/1.1, new connection per request; includes Python client overhead",
                "binary": args.binary,
                "binary_bytes": e2e.BINARY.stat().st_size,
                "platform": platform.platform(),
                "cpu": cpu,
                "logical_cpus": os.cpu_count(),
                "guest_sha256": hashlib.sha256(wasm).hexdigest(),
                "guest_bytes": len(wasm),
                "deploy_compile_seconds": round(deploy_seconds, 4),
                "requests": args.requests,
                "workers": args.workers,
                "request_body_bytes": 0,
                "response_body_bytes": results[0][2],
                "elapsed_seconds": round(elapsed, 4),
                "requests_per_second": round(args.requests / elapsed, 2),
                "latency_p50_ms": round(statistics.median(latencies), 3),
                "latency_p95_ms": round(latencies[max(0, (95 * len(latencies) + 99) // 100 - 1)], 3),
                "status_codes": codes,
                **rss,
            }, indent=2))
        finally:
            server.stop()


if __name__ == "__main__":
    main()
