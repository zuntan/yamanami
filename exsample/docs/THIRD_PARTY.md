# Third-party sources used by the demo

The sample application code is MIT, Copyright (c) 2026 zuntan. Embedded runtimes and libraries retain their own licenses.

- Common WIT in ../wit/deps/: WASI 0.2.12 definitions copied from wasmtime-wasi-http 48.0.7; upstream https://github.com/WebAssembly/WASI/tree/v0.2.12 . The WASI notice is in ../wit/LICENSE.md. The definitions are not a Yamanami-specific ABI.
- C++ vendor/mustache.hpp: kainjow/Mustache v4.1, commit c66fdd87f01f0b2d1ba704192f27355b77746a5b, Boost Software License 1.0. Full license: ../cpp/vendor/LICENSE.
- Rust: MiniJinja 2.24.0 (Apache-2.0), WASI Rust bindings and their dependencies. Exact versions and package licenses are available through ../Cargo.lock and cargo metadata.
- Python: Chevron 0.14.0 (MIT), componentize-py 0.25.0 (Apache-2.0 WITH LLVM-exception), its bundled CPython (PSF license) and standard libraries. The compiler wheel includes its prebuilt runtime; no custom Python runtime is built here.
- TypeScript: Mustache.js 4.2.0 (MIT), Jco 1.39.0 and ComponentizeJS 0.23.0 (Apache-2.0 WITH LLVM-exception), including StarlingMonkey/SpiderMonkey and their third-party components. Exact npm dependency versions are in ../typescript/package-lock.json. The runtimes' license/notice files must accompany distributed generated binaries.
- WASI SDK 34 / Clang / libc++ and wasi-libc: use their upstream notices in the downloaded SDK. wit-bindgen 0.63.0 is a build tool, not a separately installed server runtime.

Generated Wasm binaries and downloaded build tools are ignored by Git. Distributing those artifacts requires preserving the notices supplied by their dependencies; Yamanami's MIT license does not replace them.

Sources:
- https://github.com/kainjow/Mustache/tree/v4.1
- https://github.com/mitsuhiko/minijinja
- https://github.com/noahmorrison/chevron
- https://github.com/bytecodealliance/componentize-py/tree/v0.25.0
- https://github.com/bytecodealliance/ComponentizeJS
- https://github.com/janl/mustache.js
