"""Shared input loading for the contact_evidence / glossary / current_statements fixture generators.

Records are the primary user's memory_waves pairs plus every tool_cases[].prerequisite_pairs, built and upserted
exactly as harness.store.MemoryStore.seed does (one batch per wave / per tool case, `_last_per_id` within a batch).
Requests are memory_cases[].question plus tool_cases[].prompt. Contexts are built the way the agent builds them
(MEMORY_CONTEXT_PREFIX + "\\n" + json.dumps({"note", "memories": [{pairID, timestamp, text}]}, ensure_ascii=False)),
with a lexical top-k standing in for the embedding retrieval (only the add-ons are under test)."""

import glob
import json
import sys
