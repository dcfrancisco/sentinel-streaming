use clap::Parser;
use sentinel_streaming::{
    api,
    cli::{self, Cli, Command, ConfigCommand, SourceCommand},
    config::Config,
    endurance,
    frame_buffer::FrameBuffer,
    logging,
    pipeline::Pipeline,
    vision::{FrameSelector, OpenAiVisionProvider, VisionJob, VisionScheduler},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve { bind, source } => serve(bind, source).await?,
        Command::Status { endpoint } => cli::status(&endpoint).await?,
        Command::Stop { endpoint } => cli::request(reqwest::Method::POST, &endpoint, None).await?,
        Command::Version => println!("sentinel-streaming {}", env!("CARGO_PKG_VERSION")),
        Command::Source { command } => source_command(command).await?,
        Command::Config {
            command: ConfigCommand::Show { endpoint },
        } => cli::status(&endpoint).await?,
        Command::Metrics { endpoint } => cli::status(&endpoint).await?,
        Command::Endurance {
            duration,
            source,
            viewers,
            vision,
            report,
            min_fps,
        } => {
            if source != "synthetic" {
                return Err(anyhow::anyhow!("only --source synthetic is supported"));
            }
            if vision != "mock" {
                return Err(anyhow::anyhow!("only --vision mock is supported"));
            }
            let report = endurance::run(endurance::EnduranceOptions {
                duration: endurance::parse_duration(&duration)?,
                viewers,
                vision_mock: vision == "mock",
                report,
                min_fps,
            })
            .await?;
            let _ = report;
        }
        Command::Auth { command } => cli::auth(command).await?,
        Command::Profile { command } => cli::profile(command)?,
    }
    Ok(())
}

async fn serve(bind: String, source: String) -> anyhow::Result<()> {
    logging::init();
    tracing::info!("starting sentinel-streaming");
    let config = Config {
        bind,
        ..Config::default()
    };
    tracing::info!(config = ?config, "configuration loaded");
    let frame_buffer = FrameBuffer::new(config.buffer.capacity);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let state = api::AppState::new(
        config.clone(),
        frame_buffer.clone(),
        shutdown_tx.clone(),
        shutdown_tx.subscribe(),
    );
    let source_id = match source.as_str() {
        "builtin" => "builtin".to_string(),
        "synthetic" => {
            state
                .sources
                .add(sentinel_streaming::sources::AddSource {
                    id: "synthetic".into(),
                    kind: "synthetic".into(),
                    options: Default::default(),
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            "synthetic".to_string()
        }
        other => return Err(anyhow::anyhow!("unsupported initial source '{other}'")),
    };
    state
        .sources
        .start(&source_id)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    state.runtime.mark_camera_opened().await;
    tracing::info!(source = %source_id, "video source opened");
    let mut pipeline_config = config.pipeline.clone();
    pipeline_config.buffer = config.buffer.enabled;
    state.runtime.mark_pipeline_initialized().await;
    tracing::info!("pipeline initialized");
    let vision_task = if config.vision.enabled {
        VisionScheduler::spawn(VisionJob {
            buffer: frame_buffer.clone(),
            state: state.vision.clone(),
            metrics: state.vision_metrics.clone(),
            selector: FrameSelector::new(config.vision.frames, config.vision.spacing_seconds),
            interval_seconds: config.vision.interval_seconds,
            shutdown: shutdown_tx.subscribe(),
            events: state.events.clone(),
            provider: std::sync::Arc::new(
                OpenAiVisionProvider::from_env().expect("vision provider was checked above"),
            ),
            recovery: state.recovery.clone(),
        })
    } else {
        tracing::info!("vision disabled by configuration");
        None
    };
    let pipeline_state = state.clone();
    let pipeline_buffer = frame_buffer.clone();
    let pipeline_shutdown = shutdown_tx.clone();
    let mut pipeline_task = tokio::spawn(async move {
        let mut shutdown = shutdown_rx;
        loop {
            let mut pipeline = Pipeline::new(
                pipeline_state.metrics.clone(),
                pipeline_state.preview.clone(),
                pipeline_buffer.clone(),
                pipeline_config.clone(),
            );
            match pipeline
                .run(
                    pipeline_state.sources.clone(),
                    pipeline_state.clone(),
                    pipeline_shutdown.subscribe(),
                )
                .await
            {
                Ok(()) => break,
                Err(error) => {
                    let started = pipeline_state
                        .recovery
                        .begin("pipeline", error.to_string())
                        .await;
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                            pipeline_state.recovery.recovered("pipeline", started, "pipeline restarted").await;
                        }
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() { break; }
                        }
                    }
                }
            }
        }
    });
    let mut server_task = tokio::spawn(api::serve(state.clone(), shutdown_tx.subscribe()));
    tokio::select! {
        result = &mut server_task => { shutdown_tx.send(true).ok(); result??; }
        result = &mut pipeline_task => { shutdown_tx.send(true).ok(); result?; }
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
        SourceCommand::Add { id, kind, path, width, height, fps, loop_playback } => {
            cli::request(
                reqwest::Method::POST,
                &base,
                Some(serde_json::json!({"id": id, "kind": kind, "path": path, "width": width, "height": height, "fps": fps, "loop": loop_playback})),
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
        SourceCommand::Restart { id } => {
            cli::request(reqwest::Method::POST, &format!("{base}/{id}/restart"), None).await?
        }
    }
    Ok(())
}
