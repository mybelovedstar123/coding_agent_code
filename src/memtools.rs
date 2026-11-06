//! The five memory tools, served from the local store.
//!
//! The request catalog carries these tools; the harness answers them from the user's seeded store it holds (the
//! calls are also sent to `tool_endpoint`). Arguments are parsed tolerantly so a model that sends a single string
//! where a list is expected still gets a useful result.

