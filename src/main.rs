//! Serves the harness: `ditto-agent [--port N] [--env-file PATH]` (the port defaults to $PORT, then 8080).

use ditto_agent::{config, server};
