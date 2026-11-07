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

# --------------------------------------------------------------------------------------------------- edge cases
HEAD = "Case file x. Apply events by effective_step, not document order."


def rows_json(n, step_of=lambda i: i % 4, extra=""):
    return (
        "[\n"
        + ",\n".join(
            f'{{"entity": "e{i % 3}", "field": "{"members" if i % 2 else "owner"}", "value": "v{i}", '
            f'"effective_step": {step_of(i)}, "operation": "{"add" if i % 2 else "assign"}"{extra}}}'
            for i in range(n)
        )
        + "\n]"
    )


json_quirks = [
    '[{"Entity": "a", "Field": "f", "Value": 1.0, "Effective Step": 2, "Operation": "ASSIGN"}]',
    '[{"entity": "a", "field": "f", "value": NaN, "effective_step": 1, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": Infinity, "effective_step": -Infinity, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": -Infinity, "effective_step": 2.9, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": true, "effective_step": true, "operation": false}]',
    '[{"entity": "a", "field": "f", "value": null, "effective_step": 1, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": {"x": [1, 2.50, "q\'s"]}, "effective_step": 1, "operation": "x"}]',
    '[{"entity": "a", "field": "f", "value": [true, null, 1e400], "effective_step": "  ３ ", "operation": "add"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": "1_000", "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": "1__0", "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": "+7", "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": "3.0", "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": 1e300, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": 123456789012345678901234567890, "operation": "a"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": -0, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": 01, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": 1, "operation": "assign",}]',
    '[{"entity": "a", "entity": "b", "field": "f", "value": "<b>`v`</b>", "effective_step": 1, "operation": "add"}]',
    '[{"entity": "\\u00e9\\ud83d\\ude00\\n", "field": "f\\"", "value": "\\/", "effective_step": 1, "operation": "a"}]',
    '[{"entity": "a", "field": "f", "value": "tab\there", "effective_step": 1, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": "v", "effective_step": 1, "operation": "assign"}, 5, "x", []]',
    '[ {"entity": "a", "field": "f", "value": "}]", "effective_step": 1, "operation": "assign"} ]',
    'text [{"a": 1}] more [ {"entity": "a", "field": "f", "value": "v", "effective_step": 0, "operation": "add"} ]',
    '[{"entity": "a", "field": "f", "value": 1E5, "effective_step": 1, "operation": 5}]',
    '[{"entity": "a", "field": "f", "value": 1e, "effective_step": 1, "operation": "assign"}]',
    '[{"entity": "a", "field": "f", "value": -, "effective_step": 1, "operation": "assign"}]',
    '[{"entity": 12, "field": 0.1, "value": -0.0, "effective_step": 1, "operation": "assign"}]',
    '[\x1c{"entity": "a", "field": "f", "value": "v", "effective_step": 1, "operation": "assign"}]',
]
csv_quirks = [
    "entity,field,value,effective_step,operation\na,f,v,1,assign\nb,f,w,2,add\n\nc,f,x,3,assign",
    'Entity,Field,Value,Effective_Step,Operation\n"a, inc",f,"say ""hi""",1,assign\nb,f,"multi\nline",2,add',
    'entity,field,value,effective_step,operation\na,f,"open quote,1,assign\nb,f,w,2,add',
    'entity,field,value,effective_step,operation\na,f,"q"x,1,assign\na,f,v\na,f,v,1,assign,extra,more',
    "entity,field,entity,effective_step,operation,value\na,f,b,1,assign\na,f,b,1,assign,v",
    "intro, with comma\nentity,field,value,effective_step,operation\na,f,v,1,assign\neffective_step,x\n1,2",
    "entity,field,value,effective_step,operation\r\na,f,v,1,assign\r\nb,f,w,x,assign c,f,y,3,remove",
    'entity,field,value,effective_step,operation\n"",f,v,1,assign\n,,,,\na,f,v, 4 ,Assign',
    'entity,field,value,effective_step,operation\n"a"\n,f,v,1,assign',
    'entity,field,value,effective_step,operation\n"a, inc",f,"say ""hi""",1,assign\nb,f,"multi\nline,2",2,add\n'
    'c,"x""y"z,w,3,add',
    ' x,effective_step\n"open, never closed\nmore, text\n',
]
md_quirks = [
    "| Entity | Field | Value | Effective step | Operation |\n|---|---|---|---|---|\n| a | f | v | 1 | assign |\n"
    "| <code>b</code> | `f` | ` w ` | 2 | ADD |\n| | | | | |\n| c | f | x | three | assign |",
    "  | entity | field |\n| a | b |\nplain line\n| entity | field | value | effective_step | operation |\n"
    "| a | f | v | 1 | remove | extra |\n| a | f |",
    "| entity | entity | field | value | effective_step | operation |\n| :-: | --- |\n| a | b | f | v | 1 | assign |",
    "|entity|field|value|effective_step|operation\n|a|f|v|0|assign",
]
sent_quirks = [
    'At effective step 2, add "x" to the "members" set of "e1", retaining the others. '
    'At STEP 3, Remove "x" from the "members" set of "e1".',
    'For "e1", the value of "owner" becomes "p2" at effective step 8; prior values are superseded.',
    'The assignment of "v1" to field "vendor" of "e2" takes effect at step 0 and replaces its previous assignment.',
    '"e3" has "status" set to "open" at step 5.',
    '"e3" has "status" set to "open" at step 5 and "more".',
    'At step 4 "a" "b" "c" are replaced.',
    'At step 4 the "a" value "b" for "c" becomes.',
    "At step 4 nothing quoted.\nAt step 5 “x” curly.",
    'For "e1" the value of "f" becomes "v" at step 0012;\nfor "e2", the value of "g" becomes "w" at step 3.',
    'For "e1", the value of "f" becomes "v" at step ٣.',
    'For "e1", the value of "f" becomes "v" at step 5́.',
    'For "e1", the value of "f" becomes "v" at step 5‍.',
    'For "e1", the value of "f" becomes "v" at éstep 5.',
    'For "e1", the value of "f" becomes "v" at step\x1c5.',
    'For "e1", the value of "f" becomes "v" at step 5_0.',
    'x.\x1cFor "e1", the value of "f" becomes "v" at step 6. [For "e2", the value of "f" becomes "v" at step 7.',
]
for group in (json_quirks, csv_quirks, md_quirks, sent_quirks):
    add_texts(group)

edge_texts = [HEAD + "\n" + rows_json(12)]
long_csv = "entity,field,value,effective_step,operation\n" + "\n".join(
    f"e{i % 2},{'tags' if i % 3 else 'owner'},v{i},{(i * 7) % 5},{['add', 'remove', 'assign'][i % 3]}" for i in range(14)
