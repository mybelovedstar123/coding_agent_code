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
