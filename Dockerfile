# Ditto SN118 memory harness (Rust). Serves GET /health, POST /seed, POST /run on :8080.
#
# Builder: the screener-cached Rust image the official starter kits use (digest-pinned, Debian trixie).
# Runtime: python:3.12-slim (Debian trixie, glibc 2.41, as the builder). The agent is the Rust binary; CPython 3.12
# is present only for the run_python program tool, which runs model-written Python in a resource-limited child.
FROM rust:1-trixie@sha256:1bcff4befb740599103a2c7cb51058e14479b2e35e3a34a3f0dc4ede09927488 AS builder
WORKDIR /app
