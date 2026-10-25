#!/usr/bin/env python3
"""Build self-contained WASI HTTP calculator components. Run setup.py first."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
DEMO = ROOT / "exsample"
BUILD = DEMO / "build"


def run(command, **kwargs):
    print("+", *map(str, command), flush=True)
    subprocess.run(list(map(str, command)), cwd=ROOT, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools-dir", type=Path, default=DEMO / ".tools")
    parser.add_argument("--language", choices=["all", "rust", "cpp", "python", "typescript"], default="all")
    parser.add_argument("--rust-v2", action="store_true")
    args = parser.parse_args()
    tools = args.tools_dir.resolve()
    BUILD.mkdir(parents=True, exist_ok=True)
    template = (DEMO / "shared/calculator.html").read_text()
    if args.language in ("all", "rust"):
        command = ["cargo", "build", "--locked", "--manifest-path", DEMO / "Cargo.toml",
                   "--target", "wasm32-wasip2", "--release", "-j", "2"]
        if args.rust_v2:
            command += ["--features", "calculator-rust/v2"]
        run(command)
        target = DEMO / "target/wasm32-wasip2/release"
        shutil.copyfile(target / "calculator_rust.wasm", BUILD / "calculator-rust.wasm")
        shutil.copyfile(target / "calculator_index.wasm", BUILD / "index.wasm")
    if args.language in ("all", "cpp"):
        sdk = tools / "wasi-sdk-34.0-x86_64-linux/bin"
        bindgen = tools / "wit-bindgen-0.63.0-x86_64-linux/wit-bindgen"
        bindings = BUILD / "cpp-bindings"
        bindings.mkdir(exist_ok=True)
        run([bindgen, "c", DEMO / "wit", "--world", "calculator", "--out-dir", bindings])
        (bindings / "template.hpp").write_text(
            '#pragma once\nstatic constexpr const char* CALCULATOR_TEMPLATE = R"YAMANAMI('
            + template + ')YAMANAMI";\n')
        run([sdk / "clang", "--target=wasm32-wasip2", "-O2", "-c", bindings / "calculator.c",
             "-o", bindings / "bindings.o"])
        run([sdk / "clang++", "--target=wasm32-wasip2", "-std=c++20", "-O2", "-fno-exceptions",
             "-mexec-model=reactor", f"-I{bindings}", f"-I{DEMO / 'cpp'}", DEMO / "cpp/app.cpp",
             bindings / "bindings.o", bindings / "calculator_component_type.o",
             "-o", BUILD / "calculator-cpp.wasm"])
    if args.language in ("all", "python"):
        (DEMO / "python/embedded_template.py").write_text("TEMPLATE = " + repr(template) + "\n")
        run([tools / "python/bin/componentize-py", "-d", DEMO / "wit", "-w", "calculator",
             "componentize", "app", "-p", DEMO / "python",
             "-p", next((tools / "python/lib").glob("python*/site-packages")),
             "-o", BUILD / "calculator-python.wasm"])
    if args.language in ("all", "typescript"):
        bin_path = DEMO / "typescript/node_modules/.bin"
        run([bin_path / "tsc", "--noEmit", "--project", DEMO / "typescript/tsconfig.json"])
        run([bin_path / "esbuild", DEMO / "typescript/app.ts", "--bundle", "--format=esm",
             "--platform=neutral", "--external:wasi:*", "--loader:.html=text",
             f"--outfile={BUILD / 'typescript.js'}"])
        run([bin_path / "jco", "componentize", BUILD / "typescript.js", "--wit", DEMO / "wit",
             "--world-name", "calculator", "--disable", "fetch-event",
             "-o", BUILD / "calculator-typescript.wasm"])
    artifacts = {}
    for name in ["index", "calculator-rust", "calculator-cpp", "calculator-python", "calculator-typescript"]:
        file = BUILD / f"{name}.wasm"
        if file.exists():
            data = file.read_bytes()
            artifacts[name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    (BUILD / "artifacts.json").write_text(json.dumps(artifacts, indent=2) + "\n")
    print(json.dumps(artifacts, indent=2))


if __name__ == "__main__":
    main()
