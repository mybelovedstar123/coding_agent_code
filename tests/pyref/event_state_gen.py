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


texts_seen = {}
calls = []


def add_texts(texts):
    for t in texts:
        if t in texts_seen:
            continue
        texts_seen[t] = {
            "text": t,
            "from_json": evs_out(event_state.from_json(t)),
            "from_csv": evs_out(event_state.from_csv(t)),
            "from_markdown": evs_out(event_state.from_markdown(t)),
            "from_sentences": evs_out(event_state.from_sentences(t)),
        }


def add_call(source, context, ids, user_input, texts):
    add_texts(texts)
    evs = event_state.events(texts)
    step = event_state.requested_step(user_input, evs) if evs else None
    calls.append(
        {
            "source": source,
            "context": context,
            "ids": ids,
            "user_input": user_input,
            "texts": texts,
            "events": evs_out(evs),
            "requested_step": None if step is None else str(step),
            "state_at": None if step is None else json.dumps(event_state.state_at(evs, step), ensure_ascii=False),
            "expected": event_state.with_event_state(context, PREFIX, ids, user_input, texts),
        }
    )


# ------------------------------------------------------------------------------------------------- real datasets
for path in DATASETS:
    d = json.load(open(path, encoding="utf-8"))
    mem = Mem()
    batches = [w.get("pairs") or [] for w in d["memory_waves"] if w.get("user_id") == "miner"]
    batches += [c.get("prerequisite_pairs") or [] for c in d["tool_cases"]]
    for batch in batches:
        for p in batch:
            pid = str(p.get("pair_id") or "")
            mem.pairs[pid] = Pair(
                pair_id=pid,
                session_id=str(p.get("session_id") or ""),
                ts=store.parse_ts(p.get("timestamp")),
                prompt=str(p.get("prompt") or ""),
                response=str(p.get("response") or ""),
            )
    requests = [c["question"] for c in d["memory_cases"]] + [c["prompt"] for c in d["tool_cases"]]
    for q in requests:
        ident, records = request_identifier(mem, q)
        if not ident:
            continue
        add_call(
            os.path.basename(path),
            context_for(records),
            [p.pair_id for p in records],
            q,
            [p.prompt for p in records],
        )
