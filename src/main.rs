mod api;
mod auth;
mod cli;
mod config;
mod errors;
mod events;
mod frame;
mod frame_buffer;
mod health;
mod logging;
mod metrics;
mod mjpeg;
mod pipeline;
mod preview;
mod runtime;
mod sources;
mod stages;
mod vision;

use clap::Parser;
use cli::{Cli, Command, ConfigCommand, SourceCommand};
use config::Config;
use frame_buffer::FrameBuffer;
use pipeline::Pipeline;
use vision::{FrameSelector, VisionScheduler};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { bind } => serve(bind).await?,
        Command::Status { endpoint } => cli::status(&endpoint).await?,
        Command::Version => println!("sentinel-streaming {}", env!("CARGO_PKG_VERSION")),
        Command::Source { command } => source_command(command).await?,
        Command::Config {
            command: ConfigCommand::Show { endpoint },
        } => cli::status(&endpoint).await?,
        Command::Metrics { endpoint } => cli::status(&endpoint).await?,
        Command::Auth { command } => cli::auth(command).await?,
        Command::Profile { command } => cli::profile(command)?,
    }
    Ok(())
}

async fn serve(bind: String) -> anyhow::Result<()> {
    logging::init();
    tracing::info!("starting sentinel-streaming");
    let config = Config {
        bind,
        ..Config::default()
    };
    tracing::info!(config = ?config, "configuration loaded");
    let frame_buffer = FrameBuffer::new(config.buffer.capacity);
    let state = api::AppState::new(config.clone(), frame_buffer.clone());
    state
        .sources
        .start("builtin")
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    state.runtime.mark_camera_opened().await;
    tracing::info!(source = "built-in-camera", "camera opened");
    let mut pipeline_config = config.pipeline.clone();
    pipeline_config.buffer = config.buffer.enabled;
    let mut pipeline = Pipeline::new(
        state.metrics.clone(),
        state.preview.clone(),
        frame_buffer.clone(),
        pipeline_config,
    );
    state.runtime.mark_pipeline_initialized().await;
    tracing::info!("pipeline initialized");
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let vision_task = if config.vision.enabled {
        VisionScheduler::spawn(
            frame_buffer.clone(),
            state.vision.clone(),
            state.vision_metrics.clone(),
            FrameSelector::new(config.vision.frames, config.vision.spacing_seconds),
            config.vision.interval_seconds,
            shutdown_tx.subscribe(),
            state.events.clone(),
        )
    } else {
        tracing::info!("vision disabled by configuration");
        None
    };
    let pipeline_state = state.clone();
    let mut pipeline_task = tokio::spawn(async move {
        pipeline
            .run(pipeline_state.sources.clone(), pipeline_state, shutdown_rx)
            .await
    });
    let mut server_task = tokio::spawn(api::serve(state.clone(), shutdown_tx.subscribe()));
    tokio::select! {
        result = &mut server_task => { shutdown_tx.send(true).ok(); result??; }
        result = &mut pipeline_task => { shutdown_tx.send(true).ok(); result??; }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown signal received");
            state.runtime.mark_shutting_down().await;
            shutdown_tx.send(true).ok();
        }
    }
    let _ = server_task.await;
    let _ = pipeline_task.await;
    if let Some(task) = vision_task {
        let _ = task.await;
    }
    tracing::info!("sentinel-streaming stopped");
    Ok(())
}

async fn source_command(command: SourceCommand) -> anyhow::Result<()> {
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
    Ok(())
}
