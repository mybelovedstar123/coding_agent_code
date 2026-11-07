"""Differential fixtures for src/companions.rs from harness/companions.py.

Records are the 'miner' memory_waves pairs plus every tool_cases prerequisite_pairs of each v13-full dataset, in
mem.pairs insertion order. Requests are memory_cases questions plus tool_cases prompts (anchors: the records sharing
the most request phrases and words, as retrieval would rank them), plus requests made from the records that name a
joint-mention person, so companion_records fires often. Writes tests/fixtures/companions.json."""

import glob
import json
import os
import random
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import companions, store  # noqa: E402
from harness.text import tokenize  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "companions.json")
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))
rng = random.Random(118)


def load(path):
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    raw = {}
    for w in d["memory_waves"]:
        if w.get("user_id") == "miner":
            for p in w.get("pairs") or []:
                raw[p["pair_id"]] = p
    for t in d["tool_cases"]:
        for p in t.get("prerequisite_pairs") or []:
            raw[p["pair_id"]] = p
    pairs = {
        pid: store.Pair(
            pair_id=pid,
            session_id=p.get("session_id") or "",
            ts=store.parse_ts(p.get("timestamp")),
            prompt=p.get("prompt") or "",
            response=p.get("response") or "",
        )
        for pid, p in raw.items()
    }
    requests = [m["question"] for m in d["memory_cases"]] + [t["prompt"] for t in d["tool_cases"]]
    return pairs, list(dict.fromkeys(requests))


_FEATURES = {}
