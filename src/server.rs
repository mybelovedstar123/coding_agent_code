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
}
