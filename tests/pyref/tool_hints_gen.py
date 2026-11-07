"""Differential fixtures for src/tool_hints.rs from harness/tool_hints.py.

Requests: every tool_cases[].prompt and memory_cases[].question of each v13-full dataset, the request fixtures of
build-agent/tests (send / exact-value), and synthetic requests that reach each trigger's branches (typos, Unicode word
boundaries, numbers). Catalogs: each dataset's own tool_defs (built as agent.py builds them from `catalog`, with
pytool.DEFINITION appended), an empty catalog, and a synthetic catalog that offers every tool a rule names (with
descriptions that exercise the strip / rstrip(".") / [:200] formatting). Writes tests/fixtures/tool_hints.json."""

import glob
import json
import os
import random
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent")

from harness import pytool, tool_hints  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "fixtures", "tool_hints.json")
REF = "/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/build-agent"
DATASETS = sorted(glob.glob("/mnt/c/alchemist/bittensor/sn-118/ditto-subnet/submit/case/datasets/v13-full-*.json"))

WANTS = [
    "wants_schedules",
    "wants_image_edit",
    "wants_arithmetic",
    "wants_background_job",
    "wants_calendar_entry",
    "wants_sandbox_run",
    "wants_job_figure",
    "wants_note_fix",
    "wants_research_send",
    "wants_usual_effort",
    "asks_own_preference",
    "wants_send_on_record",
    "wants_exact_value",
]
CATALOG_NOTES = ["research_send_note", "effort_note", "tool_hint_note", "send_on_record_note"]
PLAIN_NOTES = ["exact_value_note", "preference_note"]


def tool_defs_for(catalog):
    wire = [t for t in catalog if isinstance(t, dict) and t.get("name")]
    defs = [
        {
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t.get("description", ""),
                "parameters": t.get("parameters") or {"type": "object", "properties": {}},
            },
        }
        for t in wire
    ]
    if pytool.TOOL_NAME not in {t["name"] for t in wire}:
        defs.append(pytool.DEFINITION)
    return defs


requests, seen = [], set()


def add_request(text, source):
    if text in seen:
        return
    seen.add(text)
    requests.append({"text": text, "source": source})
