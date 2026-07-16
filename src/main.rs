mod api;
mod auth;
mod cli;
mod config;
mod errors;
mod events;
mod frame;
mod health;
mod logging;
mod metrics;
mod pipeline;
mod preview;
mod sources;

use clap::Parser;
use cli::{AuthCommand, Cli, Command, ConfigCommand, ProfileCommand, SourceCommand};
use config::Config;
use pipeline::Pipeline;
use sources::BuiltInCamera;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Keep the executable's error boundary simple and use structured logs internally.
    logging::init();
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { bind } => {
            let config = Config {
                bind,
                ..Config::default()
            };
            let state = api::AppState::new();
            let pipeline = Pipeline::new(state.metrics.clone(), state.preview.clone());
            let source = BuiltInCamera::new(config.fps)?;
            let pipeline_task = tokio::spawn(pipeline.run(source, state.clone()));
            let server = api::serve(config, state.clone());
            tokio::select! {
                result = server => result?,
                result = pipeline_task => result??,
                _ = tokio::signal::ctrl_c() => tracing::info!("shutdown signal received"),
            }
        }
        Command::Status { endpoint } => cli::status(&endpoint).await?,
        Command::Version => println!("sentinel-streaming {}", env!("CARGO_PKG_VERSION")),
        Command::Source { command } => {
            let base = format!("{}/api/v1/sources", cli::server_base());
            match command {
                SourceCommand::List => cli::request(reqwest::Method::GET, &base, None).await?,
                SourceCommand::Add { id, kind } => {
                    cli::request(
                        reqwest::Method::POST,
                        &base,
                        Some(serde_json::json!({"id": id, "kind": kind})),
                    )
                    .await?
                }
                SourceCommand::Remove { id } => {
                    cli::request(reqwest::Method::DELETE, &format!("{base}/{id}"), None).await?
                }
                SourceCommand::Start { id } => {
                    cli::request(reqwest::Method::POST, &format!("{base}/{id}/start"), None).await?
                }
                SourceCommand::Stop { id } => {
                    cli::request(reqwest::Method::POST, &format!("{base}/{id}/stop"), None).await?
                }
            }
        }
        Command::Config {
            command: ConfigCommand::Show { endpoint },
        } => cli::status(&endpoint).await?,
        Command::Metrics { endpoint } => cli::status(&endpoint).await?,
        Command::Auth { command } => cli::auth(command).await?,
        Command::Profile { command } => cli::profile(command)?,
    }
    Ok(())
}
