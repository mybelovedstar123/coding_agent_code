# Contributing

This code is `UNLICENSED`: all rights are reserved. Please open an issue before starting work, because a contribution
can only be used with the author's agreement.

## Before opening a pull request

```sh
cargo test --locked --release
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Use `--release` for the tests: the fixtures are large and a debug build is far slower.

## Rules of the code

- `unsafe` code is not allowed.
- Follow Python's behaviour wherever a module was ported, as [PORTING.md](PORTING.md) describes.
- Declare any change to a model stage, limit or recovery rule in [PIPELINE.md](PIPELINE.md).
- Cover new behaviour with a test. Generated fixtures live in `tests/fixtures/`; see the README for how to regenerate
  them.

## Commits

Make each commit one logical change, and write its subject as `type(scope): what changed`, such as
`fix(store): keep insertion order when a pair is replaced`.
