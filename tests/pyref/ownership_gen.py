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
