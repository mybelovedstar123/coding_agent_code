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
SUBMIT = os.environ["DITTO_SUBMIT_DIR"]  # The directory holding build-agent/ and case/datasets/.
sys.path.insert(0, f"{SUBMIT}/build-agent")

from harness import companions, store  # noqa: E402
from harness.text import tokenize  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "companions.json")
DATASETS = sorted(glob.glob(f"{SUBMIT}/case/datasets/v13-full-*.json"))
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
CRAFTED_REQUESTS = [
    ("How many days was the honeymoon trip, the grape track?", ["a1"], []),
    ("How many days was the honeymoon trip, the grape track?", ["a1", "x1"], ["a1"]),
    ("How many days was the honeymoon trip, the grape track?", ["x1", "a1", "a2"], ["c2"]),
    ("What changed on the olive route, the Lisbon weekend?", ["a2"], []),
    ("What changed on the olive route weekend trip?", ["a2", "a1"], ["a2", "a1"]),
    ("Grape track trip: what is booked?", ["c3", "a1"], []),
    ("grape track", ["a1"], ["c1", "c2", "c3", "c4", "c5"]),
    ("Nothing shared here at all", ["a1", "a2"], []),
    ("honeymoon trip", [], []),
    ("honeymoon trip grape track olive route", ["a1", "a2", "c1"], []),
    ("What about the kiln plan trip?", ["a3"], []),
    ("What about the clay plan trip?", ["a4"], []),
    ("What about the glaze plan trip?", ["a5"], []),
    ("kiln plan clay plan glaze plan trip", ["a3", "a4", "a5"], []),
]


def crafted(joint=True):
    rows = CRAFTED if joint else [r for r in CRAFTED if " and I" not in r[2]]
    pairs = {pid: store.Pair(pid, "", store.parse_ts(ts), text, "") for pid, ts, text in rows}
    ordered = sorted(pairs.values(), key=store.content_key)
    index = companions.build_index(ordered)
    drop = ["c5"] if joint else []
    view = {pid: p for pid, p in pairs.items() if pid not in drop}
    cases = []
    for r, anchors, exclude in CRAFTED_REQUESTS:
        anchors = [a for a in anchors if a in pairs]
        out = companions.companion_records(index, view, r, [pairs[a] for a in anchors], set(exclude))
        cases.append({"request": r, "anchors": anchors, "exclude": exclude, "out": [p.pair_id for p in out]})
    return {
        "dataset": "crafted" if joint else "crafted-no-joint",
        "records": [[p.pair_id, p.ts.isoformat(), p.prompt, p.response] for p in pairs.values()],
        "sorted": [p.pair_id for p in ordered],
        "joint": index.joint,
        "phrase_records_len": len(index.phrase_records),
        "phrase_records_total": sum(index.phrase_records.values()),
        "drop": drop,
        "cases": cases,
        "proper_words": [[t, sorted(companions.proper_words(t))] for _, _, t in rows],
    }


def main():
    sets = []
    totals = {"cases": 0, "fired": 0}
    for path in DATASETS:
        pairs, requests = load(path)
        _FEATURES.clear()
        ordered = sorted(pairs.values(), key=store.content_key)
        index = companions.build_index(ordered)
        cases = []
        plans = [(r, None) for r in requests] + joint_requests(pairs, index, 150)
        for r, seed in plans:
            k = rng.choice([4, 8, 8, 12])
            anchors = ranked(pairs, r, k)
            if seed is not None and seed not in anchors:
                anchors.insert(rng.randrange(len(anchors) + 1), seed)
            mode = rng.random()
            if mode < 0.6:
                exclude = list(anchors)
            elif mode < 0.8:
                exclude = []
            else:
                exclude = anchors[: len(anchors) // 2] + ["not-a-pair"]
            out = companions.companion_records(index, pairs, r, [pairs[a] for a in anchors], set(exclude))
            cases.append(
                {
                    "request": r,
                    "anchors": anchors,
                    "exclude": exclude,
                    "out": [p.pair_id for p in out],
                    "phrases": sorted(list(ph) for ph in companions.phrases(r)),
                    "phrase_counts": sorted(
                        [list(ph), index.phrase_records[ph]] for ph in companions.phrases(r) if ph in index.phrase_records
                    ),
                }
            )
        # proper_words over every record prompt and the requests.
        proper = [[t, sorted(companions.proper_words(t))] for t in [p.prompt for p in pairs.values()][:300] + requests[:100]]
        sets.append(
            {
                "dataset": os.path.basename(path),
                "records": [[p.pair_id, p.ts.isoformat(), p.prompt, p.response] for p in pairs.values()],
                "sorted": [p.pair_id for p in ordered],
                "joint": index.joint,
                "phrase_records_len": len(index.phrase_records),
                "phrase_records_total": sum(index.phrase_records.values()),
                "cases": cases,
                "proper_words": proper,
            }
        )
        totals["cases"] += len(cases)
        totals["fired"] += sum(1 for c in cases if c["out"])
    sets.append(crafted())
    sets.append(crafted(joint=False))
    totals["fired"] += sum(1 for s in sets[-2:] for c in s["cases"] if c["out"])
    totals["cases"] += sum(len(s["cases"]) for s in sets[-2:])
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump({"datasets": sets}, f, ensure_ascii=False)
    print(json.dumps(totals))


if __name__ == "__main__":
    main()
