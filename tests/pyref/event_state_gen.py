"""Differential fixtures for src/event_state.rs from harness/event_state.py.

Inputs: every request (memory_cases[].question, tool_cases[].prompt) of each v13-full dataset that names an identifier
the records carry, with that identifier's records chosen as agent.request_identifier chooses them (records: the
'miner' memory_waves pairs plus every tool_cases[].prerequisite_pairs). The context is the agent's shape
(MEMORY_CONTEXT_PREFIX + "\\n" + json.dumps({"note", "memories": [{pairID, timestamp, text}]}, ensure_ascii=False)).
Plus synthetic edge cases derived from the code (JSON quirks, csv quoting, markdown, sentence roles, step parsing,
context shapes). Writes tests/fixtures/event_state.json."""

import glob
import json
import os
import sys

sys.dont_write_bytecode = True
