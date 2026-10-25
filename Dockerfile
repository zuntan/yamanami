# syntax=docker/dockerfile:1
FROM rust:1.95.0-bookworm@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 AS rust-tools
FROM node:24.22.1-bookworm-slim@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205 AS node-tools
FROM rust-tools AS server-build
ENV CARGO_BUILD_JOBS=2
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --bin yamanami \
    && install -D -m 0755 target/release/yamanami /out/yamanami
FROM python:3.12-bookworm@sha256:14a4933e6cef421424f7a66d89a3f4eafcbd0d90e0a822251f298e05132dc735 AS demo-build
COPY --from=rust-tools /usr/local/cargo /usr/local/cargo
COPY --from=rust-tools /usr/local/rustup /usr/local/rustup
COPY --from=node-tools /usr/local/bin/node /usr/local/bin/node
COPY --from=node-tools /usr/local/lib/node_modules /usr/local/lib/node_modules
ENV CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup
ENV PATH="/usr/local/cargo/bin:${PATH}"
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
WORKDIR /src
COPY rust-toolchain.toml ./
COPY exsample/scripts/setup.py ./exsample/scripts/setup.py
COPY exsample/python/requirements.txt ./exsample/python/requirements.txt
COPY exsample/typescript/package.json exsample/typescript/package-lock.json ./exsample/typescript/
RUN python3 exsample/scripts/setup.py --tools-dir /opt/demo-tools
COPY exsample ./exsample
RUN --mount=type=cache,target=/src/exsample/target \
    python3 exsample/scripts/build.py --tools-dir /opt/demo-tools
FROM debian:bookworm-slim@sha256:7ff6ba31527b9e336162fa061746c366c4b2032f4d2cff2078bea43f78efe3f8 AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 yamanami \
    && useradd --uid 10001 --gid 10001 --create-home --shell /usr/sbin/nologin yamanami \
    && install -d -o 10001 -g 10001 /var/lib/yamanami
COPY --from=server-build /out/yamanami /usr/local/bin/yamanami
COPY --chmod=0755 docker/entrypoint.sh /usr/local/bin/yamanami-entrypoint
COPY LICENSE /usr/share/doc/yamanami/LICENSE
USER 10001:10001
WORKDIR /var/lib/yamanami
EXPOSE 8080
STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/yamanami-entrypoint"]
CMD ["serve", "--listen", "0.0.0.0:8080", "--allow-non-loopback-http", "--database", "/var/lib/yamanami/yamanami.sqlite3"]
FROM runtime AS demo
COPY --from=demo-build /src/exsample/build/*.wasm /opt/yamanami/apps/
COPY exsample/docs/THIRD_PARTY.md /usr/share/doc/yamanami/DEMO-THIRD-PARTY.md
COPY exsample/cpp/vendor/LICENSE /usr/share/doc/yamanami/MUSTACHE-LICENSE
ENV YAMANAMI_DEPLOY_DEMO=1 RAYON_NUM_THREADS=2
CMD ["serve", "--listen", "0.0.0.0:8080", "--allow-non-loopback-http", "--database", "/var/lib/yamanami/yamanami.sqlite3", "--upload-limit", "33554432", "--compiler-opt-level", "none", "--memory-mib", "256", "--max-memories", "8", "--max-instances", "128", "--max-tables", "32", "--fuel", "1000000000", "--timeout-ms", "10000", "--concurrency", "8", "--app-concurrency", "2"]
