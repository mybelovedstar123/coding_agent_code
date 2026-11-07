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
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import config, event_state, store  # noqa: E402
from harness.agent import request_identifier  # noqa: E402
from harness.store import Pair, fmt_ts, render_pair  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "event_state.json")
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))
PREFIX = config.MEMORY_CONTEXT_PREFIX
NOTE = "Stored records for the identifier in the request (oldest first)."


class Mem:
    def __init__(self):
        self.pairs = {}


def evs_out(evs):
    return [[str(s), op, e, f, v] for s, op, e, f, v in evs]


def safe(fn, *a):
    try:
        return {"ok": fn(*a)}
    except Exception as exc:  # noqa: BLE001
        return {"raises": type(exc).__name__}


def context_for(records, note=NOTE):
    payload = {
        "note": note,
        "memories": [{"pairID": p.pair_id, "timestamp": fmt_ts(p.ts), "text": render_pair(p)} for p in records],
    }
    return PREFIX + "\n" + json.dumps(payload, ensure_ascii=False)
