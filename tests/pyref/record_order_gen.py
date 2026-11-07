"""Differential fixtures for src/record_order.rs from harness/record_order.py.

Records are the 'miner' memory_waves pairs plus every tool_cases prerequisite_pairs of each v13-full dataset. Cases
are the agent's call (`copies_note` over a request's identifier records, chosen as agent.request_identifier does) for
every memory_cases question and tool_cases prompt, plus every identifier the records carry, plus crafted copy sets.
Writes tests/fixtures/record_order.json."""

import glob
import json
import os
import random
import re
import sys
from datetime import datetime, timedelta, timezone

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import record_order as ro, store  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "record_order.json")
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))
# agent.py
rng = random.Random(118)
IDENT_RE = re.compile(r"\b(?=[A-Za-z0-9-]*\d)(?=[A-Za-z0-9-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9-]{7,}\b")
