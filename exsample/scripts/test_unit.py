#!/usr/bin/env python3
"""Native arithmetic unit tests in all four source languages."""
from pathlib import Path
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "exsample"
BUILD = DEMO / "build"
BUILD.mkdir(exist_ok=True)

def run(command, cwd=ROOT):
    subprocess.run(list(map(str, command)), cwd=cwd, check=True)

run(["cargo", "test", "--locked", "--manifest-path", DEMO / "Cargo.toml", "-p", "calculator-rust"])
run(["c++", "-std=c++20", "-Wall", "-Wextra", "-Werror", "-I" + str(DEMO / "cpp"),
     DEMO / "cpp/test.cpp", "-o", BUILD / "cpp-unit"])
run([BUILD / "cpp-unit"])
run([sys.executable, "-m", "unittest", "discover", "-s", DEMO / "python/test", "-v"])
run(["npm", "test"], cwd=DEMO / "typescript")
