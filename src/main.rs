//! Serves the harness: `ditto-agent [--port N] [--env-file PATH]` (the port defaults to $PORT, then 8080).

use ditto_agent::{config, server};

fn main() -> anyhow::Result<()> {
    let mut port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().and_then(|p| p.parse().ok()).unwrap_or(port),
            "--env-file" => {
                if let Some(path) = args.next().filter(|p| !p.is_empty()) {
                    config::load_dotenv(std::path::Path::new(&path));
                }
            }
            other => anyhow::bail!("unknown argument {other:?} (usage: ditto-agent [--port N] [--env-file PATH])"),
        }
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
            tracing::info!("listening on :{port}");
            axum::serve(listener, server::router(server::App::new())).await?;
            Ok(())
        })
}
