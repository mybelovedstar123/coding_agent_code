"""Fixtures for src/current_statements.rs from harness/current_statements.py (the reference).

Cases:
- every request of every v13-full dataset that names an identifier the records carry (agent.request_identifier):
  the identifier records as (pairID, prompt) items, the agent-built context and every function's output;
- every identifier the records carry (IDENT_RE over the records, lower-cased; at most SWEEP_MAX records), with the
  request set to the records' own statements (so every change note is on topic) over a minimal context;
- difflib ratios over record statements (pairs, typo'd copies, 200+ character texts for the autojunk rule), the
  fuzzy cue matcher on typo'd cues, and synthetic list changes.
Writes tests/fixtures/current_statements.json."""

import difflib
import os
import random
import sys

sys.dont_write_bytecode = True
