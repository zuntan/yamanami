#!/usr/bin/env python3
"""Deploy all five applications via the yamanami CLI."""
import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
APPLICATIONS = [("index", "/"), ("calculator-rust", "/rust"), ("calculator-cpp", "/cpp"),
                ("calculator-python", "/python"), ("calculator-typescript", "/typescript")]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/yamanami")
    parser.add_argument("--endpoint", default="http://127.0.0.1:9090")
    args = parser.parse_args()
    if not os.environ.get("YAMANAMI_TOKEN"):
        parser.error("set YAMANAMI_TOKEN to the running server's management token")
    for app, path in APPLICATIONS:
        subprocess.run([str(args.binary.resolve()), "ctl", "--endpoint", args.endpoint, "deploy",
                        str(ROOT / f"exsample/build/{app}.wasm"), "--app", app, "--path", path],
                       cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
