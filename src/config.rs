//! Runtime configuration, read from the environment on every lookup.
//!
//! On-chain the scorer injects the inference relay (per case via `inference_base_url`, or process-wide via
//! `DITTOBENCH_INFERENCE_BASE_URL`) and an Ollama-compatible embedding gateway via `OLLAMA_BASE_URL`. Locally the
//! same code talks to OpenRouter and a local Ollama.

pub const DEFAULT_MODEL: &str = "openai/gpt-oss-20b";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
