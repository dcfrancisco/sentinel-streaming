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
        Command::CheckConfig { config } => check_config(&config)?,
        Command::Doctor { config } => doctor(&config)?,
        Command::SupportBundle {
            endpoint,
            output,
            logs,
        } => cli::support_bundle(&endpoint, &output, logs.as_deref()).await?,
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

fn check_config(path: &std::path::Path) -> anyhow::Result<()> {
    let config = Config::load(path, None, None)
        .map_err(|error| anyhow::anyhow!("Configuration is invalid: {error}"))?;
    println!("Configuration valid: {}", path.display());
    println!("Bind address: {}", config.bind);
    println!("Instance ID: {}", config.instance_id);
    println!("Deployment profile: {}", config.deployment_profile);
    println!(
        "MediaGateway: {} ({})",
        config.media_gateway.kind,
        if config.media_gateway.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("Sources: {}", config.sources.len());
    Ok(())
}

fn doctor(path: &std::path::Path) -> anyhow::Result<()> {
    let config = Config::load(path, None, None)
        .map_err(|error| anyhow::anyhow!("Configuration is invalid: {error}"))?;
    println!("Sentinel Streaming doctor");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Configuration: PASS ({})", path.display());
    println!("Security mode: {}", config.security.mode.as_str());
    if config.security.mode.as_str() == "OPEN_LOCAL_TEST" {
        println!("Warning: local test mode disables authentication and is loopback-only");
    }
    println!("MediaGateway type: {}", config.media_gateway.kind);
    if config.media_gateway.enabled {
        for (name, value) in [
            ("MediaMTX API", config.media_gateway.api_url.as_deref()),
            (
                "WebRTC base",
                config.media_gateway.webrtc_base_url.as_deref(),
            ),
            ("HLS base", config.media_gateway.hls_base_url.as_deref()),
        ] {
            println!(
                "{name}: {}",
                value
                    .map(redact_diagnostic_url)
                    .unwrap_or_else(|| "not configured".into())
            );
        }
    } else {
        println!("MediaGateway: disabled (basic installation does not require MediaMTX)");
    }
    println!("Admin URL: http://{}/admin", config.bind);
    Ok(())
}

fn redact_diagnostic_url(value: &str) -> String {
    url::Url::parse(value)
        .map(|mut parsed| {
            let _ = parsed.set_username("[REDACTED]");
            let _ = parsed.set_password(Some("[REDACTED]"));
            parsed.to_string()
        })
        .unwrap_or_else(|_| "[REDACTED_URL]".into())
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
        tracing::info!("no enabled video sources configured; starting in camera-free setup mode");
    }
    let health_task = state.sources.spawn_health_monitor();
    let media_supervision_task = state.sources.spawn_media_supervisor();
    let mut pipeline_config = config.pipeline.clone();
    pipeline_config.buffer = config.buffer.enabled;
    state.runtime.mark_pipeline_initialized().await;
    // Service readiness means the Sentinel API and control plane are available.
    // Camera/media readiness remains source-scoped and is reported separately.
    state
        .health
        .ready
        .store(true, std::sync::atomic::Ordering::Relaxed);
    tracing::info!("pipeline initialized");
    let vision_task = if config.vision.enabled {
        match OpenAiVisionProvider::from_env() {
            Some(provider) => VisionScheduler::spawn(VisionJob {
                buffer: frame_buffer.clone(),
                state: state.vision.clone(),
                metrics: state.vision_metrics.clone(),
                selector: FrameSelector::new(config.vision.frames, config.vision.spacing_seconds),
                interval_seconds: config.vision.interval_seconds,
                shutdown: shutdown_tx.subscribe(),
                events: state.events.clone(),
                provider: std::sync::Arc::new(provider),
                recovery: state.recovery.clone(),
            }),
            None => {
                tracing::warn!("OpenAI Vision disabled; OPENAI_API_KEY is unavailable");
                None
            }
        }
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
    state.sources.shutdown_sources().await;
    let _ = server_task.await;
    let _ = pipeline_task.await;
    state.sources.shutdown_media_gateway().await;
    if let Some(task) = health_task {
        let _ = task.await;
    }
    if let Some(task) = media_supervision_task {
        let _ = task.await;
    }
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
