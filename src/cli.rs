use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "sentinel-streaming",
    version,
    about = "Sentinel headless streaming engine"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand, Debug)]
pub enum Command {
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long, default_value = "sentinel.yaml")]
        config: std::path::PathBuf,
        #[arg(
            long,
            help = "override configured initial source: builtin or synthetic"
        )]
        source: Option<String>,
    },
    Status {
        #[arg(long, default_value = "http://127.0.0.1:8080/api/v1/status")]
        endpoint: String,
    },
    Stop {
        #[arg(long, default_value = "http://127.0.0.1:8080/api/v1/stop")]
        endpoint: String,
    },
    Version,
    CheckConfig {
        #[arg(long, default_value = "sentinel.yaml")]
        config: std::path::PathBuf,
    },
    Doctor {
        #[arg(long, default_value = "sentinel.yaml")]
        config: std::path::PathBuf,
    },
    SupportBundle {
        #[arg(long, default_value = "http://127.0.0.1:8080/api/v1/support/bundle")]
        endpoint: String,
        #[arg(long, default_value = "support-bundle")]
        output: std::path::PathBuf,
        #[arg(long)]
        logs: Option<std::path::PathBuf>,
    },
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Metrics {
        #[arg(long, default_value = "http://127.0.0.1:8080/metrics")]
        endpoint: String,
    },
    Endurance {
        #[arg(long, default_value = "60s")]
        duration: String,
        #[arg(long, default_value = "synthetic")]
        source: String,
        #[arg(long, default_value_t = 5)]
        viewers: usize,
        #[arg(long, default_value = "mock")]
        vision: String,
        #[arg(long)]
        report: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = 20.0)]
        min_fps: f64,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
}
#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    Login {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        server: String,
    },
    Logout {
        #[arg(long)]
        profile: Option<String>,
    },
    Status,
    Whoami,
}
#[derive(Subcommand, Debug)]
pub enum ProfileCommand {
    List,
    Use {
        profile: String,
    },
    Add {
        profile: String,
        #[arg(long)]
        server: String,
    },
}
#[derive(Subcommand, Debug)]
pub enum SourceCommand {
    List,
    Add {
        #[arg(required = false)]
        id: Option<String>,
        #[arg(long = "id")]
        id_flag: Option<String>,
        #[arg(long, default_value = "built-in-camera", alias = "type")]
        kind: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long)]
        uri: Option<String>,
        #[arg(long)]
        transport: Option<String>,
        #[arg(long)]
        username_env: Option<String>,
        #[arg(long)]
        password_env: Option<String>,
        #[arg(long, default_value_t = true)]
        loop_playback: bool,
    },
    Remove {
        id: String,
    },
    Start {
        id: String,
    },
    Stop {
        id: String,
    },
    Restart {
        id: String,
    },
}
#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    Show {
        #[arg(long, default_value = "http://127.0.0.1:8080/api/v1/config")]
        endpoint: String,
    },
}

pub async fn request(
    method: reqwest::Method,
    endpoint: &str,
    body: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let mut request = client.request(method, endpoint);
    if let Ok((_, _, token)) = crate::auth::current() {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await?;
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!("request failed ({status}): {text}"));
    }
    println!("{text}");
    Ok(())
}
pub async fn status(endpoint: &str) -> anyhow::Result<()> {
    request(reqwest::Method::GET, endpoint, None).await
}

pub async fn support_bundle(
    endpoint: &str,
    output: &std::path::Path,
    logs: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    if output.exists() {
        return Err(anyhow::anyhow!(
            "support-bundle output already exists: {}",
            output.display()
        ));
    }
    let client = reqwest::Client::new();
    let mut request = client.get(endpoint);
    if let Ok((_, _, token)) = crate::auth::current() {
        request = request.bearer_auth(token);
    }
    let response = request.send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "support bundle request failed ({status}): {body}"
        ));
    }
    let snapshot: serde_json::Value = serde_json::from_str(&body)?;
    write_support_bundle(output, &snapshot, logs)?;
    println!("Support bundle written to {}", output.display());
    Ok(())
}

fn write_support_bundle(
    output: &std::path::Path,
    snapshot: &serde_json::Value,
    logs: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(output.join("logs"))?;
    write_json(output.join("manifest.json"), &snapshot["manifest"])?;
    write_json(output.join("version.json"), &snapshot["version"])?;
    write_json(output.join("health.json"), &snapshot["health"])?;
    write_json(
        output.join("source-summary.json"),
        &snapshot["sourceSummary"],
    )?;
    write_json(
        output.join("dependency-health.json"),
        &snapshot["dependencyHealth"],
    )?;

    let config = serde_yaml::to_string(&snapshot["sanitizedConfig"])?;
    std::fs::write(output.join("sanitized-config.yaml"), config)?;

    let mut events = String::new();
    if let Some(values) = snapshot["recentOperationalEvents"].as_array() {
        for value in values {
            events.push_str(&serde_json::to_string(value)?);
            events.push('\n');
        }
    }
    std::fs::write(output.join("recent-operational-events.jsonl"), events)?;

    if let Some(log_path) = logs {
        if !log_path.is_file() {
            return Err(anyhow::anyhow!(
                "log path is not a file: {}",
                log_path.display()
            ));
        }
        let name = log_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("log path has no file name"))?;
        let content = std::fs::read_to_string(log_path)?;
        let sanitized = content
            .lines()
            .map(sanitize_log_line)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(output.join("logs").join(name), format!("{sanitized}\n"))?;
    }
    Ok(())
}

fn write_json(path: std::path::PathBuf, value: &serde_json::Value) -> anyhow::Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn sanitize_log_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if [
        "authorization:",
        "bearer ",
        "password",
        "secret",
        "bootstrap_token",
        "api_token",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        "[REDACTED_LOG_LINE]".into()
    } else {
        line.into()
    }
}
pub fn server_base() -> String {
    crate::auth::current()
        .map(|(_, profile, _)| profile.server)
        .unwrap_or_else(|_| "http://127.0.0.1:8080".into())
}

pub async fn auth(command: AuthCommand) -> anyhow::Result<()> {
    match command {
        AuthCommand::Login { profile, server } => {
            let token = rpassword::prompt_password("API token: ")?;
            crate::auth::login(&profile, &server, &token)?;
            println!("Logged in as profile '{profile}'.");
        }
        AuthCommand::Logout { profile } => {
            crate::auth::logout(profile.as_deref())?;
            println!("Logged out.");
        }
        AuthCommand::Status => {
            let (current, profiles) = crate::auth::list()?;
            println!(
                "active: {}\nprofiles: {}",
                current.unwrap_or_else(|| "none".into()),
                profiles.len()
            );
        }
        AuthCommand::Whoami => {
            let (_, profile, _) = crate::auth::current()?;
            request(
                reqwest::Method::GET,
                &format!("{}/api/v1/auth/whoami", profile.server),
                None,
            )
            .await?;
        }
    }
    Ok(())
}
pub fn profile(command: ProfileCommand) -> anyhow::Result<()> {
    match command {
        ProfileCommand::List => {
            let (current, profiles) = crate::auth::list()?;
            for name in profiles.keys() {
                println!(
                    "{}{}",
                    if current.as_deref() == Some(name) {
                        "* "
                    } else {
                        "  "
                    },
                    name
                );
            }
        }
        ProfileCommand::Use { profile } => {
            crate::auth::use_profile(&profile)?;
            println!("Using profile '{profile}'.");
        }
        ProfileCommand::Add { profile, server } => {
            crate::auth::add_profile(&profile, &server)?;
            println!("Profile '{profile}' added. Run auth login to store credentials.");
        }
    }
    Ok(())
}
