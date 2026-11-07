"""Fixtures for src/contact_evidence.rs from harness/contact_evidence.py (the reference).

For every request of every v13-full dataset (and a set of synthetic edge requests over a small crafted memory), the
expected output of every public function, plus the agent's contact add-on chain (agent._memory_context order) over a
context built the way the agent builds it. Writes tests/fixtures/contact_evidence.json."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ce_records as R  # noqa: E402

from harness import contact_evidence as ce  # noqa: E402
from harness.store import fmt_ts, render_pair  # noqa: E402

PREFIX = R.PREFIX


def attempt(fn):
    """{"value": result} or {"raises": exception type} (the Rust port documents its fallback for these)."""
    try:
        return {"value": fn()}
    except Exception as err:  # noqa: BLE001
        return {"raises": type(err).__name__}
