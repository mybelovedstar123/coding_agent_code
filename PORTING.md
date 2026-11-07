# Porting contract: Python reference → Rust

This crate is a faithful port of the Python agent in `submit/build-agent` (tag `v14`; the reference is the working
tree's `harness/` package). The Rust agent must send the model **exactly** the requests the Python agent sends for
the same case. Every model call is cached by request content, so any difference in a prompt is a cache miss, a paid
call and an unmeasured behaviour change. The port is proven by replaying cached runs with zero misses.

## Rules

1. **One Python module, one Rust module**, with the same name and the same public functions (snake_case, same
   argument order). Constants keep their names (`UPPER_CASE`). Docstrings become `///` doc comments; keep the
   explanatory comments.
