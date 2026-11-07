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


def ranked(pairs, request, k):
    """The k records sharing the most request phrases, then words (a stand-in for retrieval's hits)."""
    rp = companions.phrases(request)
    rw = set(tokenize(request))
    scored = []
    for i, (pid, p) in enumerate(pairs.items()):
        if pid not in _FEATURES:
            _FEATURES[pid] = (companions.phrases(p.prompt), set(tokenize(p.prompt)))
        fp, fw = _FEATURES[pid]
        ph = len(rp & fp)
        w = len(rw & fw)
        if ph or w:
            scored.append((-ph, -w, i, pid))
    scored.sort()
    return [pid for _, _, _, pid in scored[:k]]


def joint_requests(pairs, index, n):
    """Requests made from records that name a joint-mention person: a span of the record's words."""
    named = []
    for pid, p in pairs.items():
        for name in index.joint:
            if name in p.prompt:
                named.append(pid)
                break
    rng.shuffle(named)
    out = []
    for pid in named[:n]:
        words = pairs[pid].prompt.split()
        if len(words) < 4:
            continue
        i = rng.randrange(max(1, len(words) - 10))
        span = " ".join(words[i : i + rng.randrange(4, 14)])
        q = rng.choice(["What's the latest on {s}?", "{s} - where did that land?", "Remind me: {s}", "{s}"])
        out.append((q.format(s=span), pid))
    return out


CRAFTED = [
    ("a1", "2025-01-01T09:00:00Z", "Our grape track was the honeymoon trip to Porto that I planned with Sparky."),
    ("a2", "2025-01-02T09:00:00Z", "The olive route is the Lisbon weekend I planned with Sparky and Mo."),
    ("c1", "2025-02-01T09:00:00Z", "When Sparky and I first mapped that trip out, we had 8 days in Porto."),
    ("c2", "2025-02-02T09:00:00Z", "Quick update on the trip Sparky and I planned: Porto hotel is booked."),
    ("c3", "2025-02-03T09:00:00Z", "Sparky and I changed the trip: two more days in Porto and Braga."),
    ("c4", "2025-02-04T09:00:00Z", "Sparky and I and Mo and I argued about the trip budget for Porto."),
    ("c5", "2025-02-05T09:00:00Z", "Sparky and I moved the trip flights; Porto arrival is now Friday."),
    ("c6", "2025-02-06T09:00:00Z", "Mo and I priced the Lisbon weekend trains."),
    ("c7", "2025-02-07T09:00:00Z", "Sparky and I booked dinner (no place named), trip notes later."),
    ("c8", "2025-02-08T09:00:00Z", "Sparky and Ivan and I drove to Porto for the trip."),
    ("x1", "2025-03-01T09:00:00Z", "Sparky said the grape track budget is fine. Porto"),
    ("x2", "2025-03-02T09:00:00Z", "Sparkys and I is not a joint mention; Porto trip."),
    ("x3", "2025-03-03T09:00:00Z", "Porto and I start a record here: Sparky trip."),
    ("a3", "2025-01-03T09:00:00Z", "The kiln plan with Sparky2 and Sparky_x and xSparky in Porto."),
    ("a4", "2025-01-04T09:00:00Z", "The clay plan with (Sparky) in Porto, Braga."),
    ("a5", "2025-01-05T09:00:00Z", "Sparky: the glaze plan in Porto."),
]
