"""Fixtures for src/glossary.rs from harness/glossary.py (the reference).

Cases:
- every request of every v13-full dataset that names an identifier the records carry (agent.request_identifier):
  the identifier records as (pairID, prompt) items, the agent-built context, and every function's output;
- every identifier the records carry (IDENT_RE over the records, lower-cased), as if a request named it alone, over a
  minimal context: the sweep that reaches every glossary-defining case file;
- synthetic edge texts and contexts.
Writes tests/fixtures/glossary.json."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ce_records as R  # noqa: E402

from harness import glossary as g  # noqa: E402

PREFIX = R.PREFIX
