"""Writes src/prompts.rs: every prompt and note text of the Python reference as a Rust literal, byte-identical.

Run from WSL: python3 tests/pyref/gen_prompts.py (reads submit/build-agent/harness)."""

import sys
from pathlib import Path
