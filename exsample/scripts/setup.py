#!/usr/bin/env python3
"""Install pinned demo build tools locally (Linux x86_64, Python 3.12+, Node 24)."""
import argparse
import hashlib
from pathlib import Path
import platform
import subprocess
import tarfile
import urllib.request
import venv

ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "exsample"
ASSETS = [
    ("sdk", "wasi-sdk-34.0-x86_64-linux",
     "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-x86_64-linux.tar.gz",
     "b761e3a0721dbae9c09a0059e5fdb2bf917d1b4a8a7b430fb3b5aafb0984b2c4"),
    ("bindgen", "wit-bindgen-0.63.0-x86_64-linux",
     "https://github.com/bytecodealliance/wit-bindgen/releases/download/v0.63.0/wit-bindgen-0.63.0-x86_64-linux.tar.gz",
     "bf4efe883ab70d34a4740253911dda20caf895d58a003af58369506b04684cbe"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools-dir", type=Path, default=DEMO / ".tools")
    args = parser.parse_args()
    tools = args.tools_dir.resolve()
    if platform.system() != "Linux" or platform.machine() not in ("x86_64", "AMD64"):
        parser.error("this setup script currently supports Linux x86_64 only")
    tools.mkdir(parents=True, exist_ok=True)
    for name, directory, url, expected in ASSETS:
        archive = tools / f"{name}.tar.gz"
        if (tools / directory).exists():
            continue
        print(f"Downloading {directory}", flush=True)
        urllib.request.urlretrieve(url, archive)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"checksum mismatch: {archive}")
        with tarfile.open(archive) as source:
            source.extractall(tools, filter="data")
    if not (tools / "python/bin/python").exists():
        venv.create(tools / "python", with_pip=True)
    subprocess.run([str(tools / "python/bin/pip"), "install", "-r",
                    str(DEMO / "python/requirements.txt")], check=True)
    subprocess.run(["npm", "ci", "--no-audit", "--no-fund"], cwd=DEMO / "typescript", check=True)
    subprocess.run(["rustup", "show"], cwd=ROOT, check=True)
    print(f"Tools ready: {tools}")


if __name__ == "__main__":
    main()
