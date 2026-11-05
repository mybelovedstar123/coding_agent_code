//! Runtime configuration, read from the environment on every lookup.
//!
//! On-chain the scorer injects the inference relay (per case via `inference_base_url`, or process-wide via
//! `DITTOBENCH_INFERENCE_BASE_URL`) and an Ollama-compatible embedding gateway via `OLLAMA_BASE_URL`. Locally the
//! same code talks to OpenRouter and a local Ollama.

pub const DEFAULT_MODEL: &str = "openai/gpt-oss-20b";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const EMBED_MODEL: &str = "embeddinggemma";
pub const EMBED_DIM: usize = 768;
/// PROTOCOL.md (/run and /seed): user_id is optional and the documented default memory graph is "miner". A request
/// without user_id therefore belongs to that single default principal (policy v13 W15: "a documented default user
/// represents one default principal"); requests that carry a user_id are always scoped to it.
pub const DEFAULT_USER: &str = "miner";
