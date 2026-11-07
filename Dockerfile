# Ditto SN118 memory harness (Rust). Serves GET /health, POST /seed, POST /run on :8080.
#
# Builder: the screener-cached Rust image the official starter kits use (digest-pinned, Debian trixie).
# Runtime: python:3.12-slim (Debian trixie, glibc 2.41, as the builder). The agent is the Rust binary; CPython 3.12
# is present only for the run_python program tool, which runs model-written Python in a resource-limited child.
FROM rust:1-trixie@sha256:1bcff4befb740599103a2c7cb51058e14479b2e35e3a34a3f0dc4ede09927488 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Compile the locked dependency graph before copying the source, so a source change reuses this layer.
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && printf '\n' > src/lib.rs \
    && cargo build --locked --release \
    && rm -rf src
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --locked --release --bin ditto-agent

FROM python:3.12-slim@sha256:a6e34c598f2467ed0e9a8d349809fcd8b5c603269512df273a0bb1784edc11b1
COPY --from=builder /app/target/release/ditto-agent /usr/local/bin/ditto-agent
