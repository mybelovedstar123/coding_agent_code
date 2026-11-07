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
