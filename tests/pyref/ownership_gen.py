"""Differential fixtures for src/ownership.rs from harness/ownership.py.

Records are the 'miner' memory_waves pairs plus every tool_cases prerequisite_pairs of each v13-full dataset (in
mem.pairs insertion order: first insertion position, latest value). Requests are memory_cases questions plus
tool_cases prompts, plus synthetic holder questions built from the records' handover sentences so the ownership path
fires often. Writes tests/fixtures/ownership.json."""

import difflib
import glob
import json
import os
import random
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import config, ownership, store  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "ownership.json")
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))
PREFIX = config.MEMORY_CONTEXT_PREFIX
rng = random.Random(118)
# agent.py IDENT_RE: requests naming a case-file identifier never reach with_ownership (the agent calls it only when
# the request has no identifier records), and best_chain over their long event-history records is slow in Python;
# only a sample of them is run through best_chain.
IDENT_RE = re.compile(r"\b(?=[A-Za-z0-9-]*\d)(?=[A-Za-z0-9-]*[A-Za-z])[A-Za-z0-9][A-Za-z0-9-]{7,}\b")
IDENT_SAMPLE = 6


def memo_best_chain(idx):
    """best_chain memoised per request (same results; it is a pure function of the index and the request)."""
    orig = idx.best_chain
    cache = {}

    def best_chain(request):
        if request not in cache:
            cache[request] = orig(request)
        return cache[request]

    idx.best_chain = best_chain


class P:
    def __init__(self, d):
        self.pair_id = d["pair_id"]
        self.ts = store.parse_ts(d.get("timestamp"))
        self.prompt = d.get("prompt") or ""
        self.response = d.get("response") or ""


def load(path):
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    pairs = {}
    for w in d["memory_waves"]:
        if w.get("user_id") == "miner":
            for p in w.get("pairs") or []:
                pairs[p["pair_id"]] = P(p)
    for t in d["tool_cases"]:
        for p in t.get("prerequisite_pairs") or []:
            pairs[p["pair_id"]] = P(p)
    requests = [m["question"] for m in d["memory_cases"]] + [t["prompt"] for t in d["tool_cases"]]
    return pairs, list(dict.fromkeys(requests))


def chain_list(ch):
    return [[ts, s] for ts, s in ch]
