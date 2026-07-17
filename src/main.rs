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
        Command::Serve {
            bind,
            config,
            source,
        } => serve(bind, config, source).await?,
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

async fn serve(
    bind: Option<String>,
    config_path: std::path::PathBuf,
    source: Option<String>,
) -> anyhow::Result<()> {
    let config = Config::load(&config_path, bind.as_deref(), source.as_deref())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    logging::init(&config.logging.level);
    tracing::info!("starting sentinel-streaming");
    tracing::info!(config = ?config, "configuration loaded");
    let frame_buffer = FrameBuffer::new(config.buffer.capacity);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let state = api::AppState::new(
        config.clone(),
        frame_buffer.clone(),
        shutdown_tx.clone(),
        shutdown_tx.subscribe(),
    );
    let mut started_source = None;
    for configured in &config.sources {
        if configured.id != "builtin" {
            state
                .sources
                .add(sentinel_streaming::sources::AddSource {
                    id: configured.id.clone(),
                    kind: configured.kind.clone(),
                    options: configured.options(),
                })
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        if configured.enabled && started_source.is_none() {
            state
                .sources
                .start(&configured.id)
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            started_source = Some(configured.id.clone());
            state.runtime.mark_camera_opened().await;
            tracing::info!(source = %configured.id, "video source opened");
        }
    }
    if started_source.is_none() {
        return Err(anyhow::anyhow!("configuration has no enabled video source"));
    }
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
        SourceCommand::Add {
            id,
            id_flag,
            kind,
            path,
            width,
            height,
            fps,
            loop_playback,
            uri,
            transport,
            username_env,
            password_env,
        } => {
            let id = id_flag
                .or(id)
                .ok_or_else(|| anyhow::anyhow!("source add requires <id> or --id"))?;
            cli::request(
                reqwest::Method::POST,
                &base,
                Some(serde_json::json!({"id": id, "kind": kind, "path": path, "width": width, "height": height, "fps": fps, "loop": loop_playback, "uri": uri, "transport": transport, "credentials": {"username_env": username_env, "password_env": password_env}})),
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
