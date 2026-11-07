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
