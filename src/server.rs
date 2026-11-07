//! HTTP server: GET /health, POST /seed, POST /run on port 8080.

use crate::agent::Agent;
use crate::config;
use crate::embed::{Embed, Embedder};
use crate::llm::{ChatClient, HttpChat};
use crate::store::{truthy, Store};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct App {
    pub store: Arc<Store>,
    pub agent: Arc<Agent>,
}

impl App {
    /// The production wiring: the embedding gateway and the inference relay over one HTTP client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(128)
            .build()
            .expect("http client");
        let embedder: Arc<dyn Embed> = Arc::new(Embedder::new(http.clone()));
        let chat: Arc<dyn ChatClient> = Arc::new(HttpChat::new(http.clone()));
        Self::with(embedder, chat, http)
    }

    /// Wiring with injected dependencies (tests and the equivalence harness use fakes).
    pub fn with(embedder: Arc<dyn Embed>, chat: Arc<dyn ChatClient>, http: reqwest::Client) -> Self {
        let store = Arc::new(Store::new(embedder));
        let agent = Arc::new(Agent::new(store.clone(), chat, http));
        App { store, agent }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn error(status: StatusCode, message: String) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}

/// The request body as a JSON object, or the reference's 400 response.
fn object_body(raw: &[u8]) -> Result<serde_json::Map<String, Value>, Box<Response>> {
    match serde_json::from_slice::<Value>(raw) {
        Err(_) => Err(Box::new(error(StatusCode::BAD_REQUEST, "invalid JSON".into()))),
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err(Box::new(error(
            StatusCode::BAD_REQUEST,
            "the body must be a JSON object".into(),
        ))),
    }
}
