# Ditto memory agent (Rust)

![Rust 2021](https://img.shields.io/badge/Rust-2021_edition-orange?logo=rust)
![Axum](https://img.shields.io/badge/HTTP-axum_0.7-blue)
![Tokio](https://img.shields.io/badge/runtime-tokio-463aa1)
![Docker](https://img.shields.io/badge/image-digest--pinned-2496ed?logo=docker&logoColor=white)
![unsafe: none](https://img.shields.io/badge/unsafe-none-brightgreen)

A memory agent harness for the Ditto subnet (Bittensor SN118). It serves an HTTP API, holds each user's
records in memory, retrieves evidence for a request, and runs a bounded loop between a language model and tools until
it has an answer.

