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
        #[arg(long, default_value = "0.0.0.0:8080")]
        bind: String,
    },
    Status {
        #[arg(long, default_value = "http://127.0.0.1:8080/api/v1/status")]
        endpoint: String,
    },
    Version,
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
        id: String,
        #[arg(long, default_value = "built-in-camera")]
        kind: String,
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
) -> Result<(), Box<dyn std::error::Error>> {
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
        return Err(format!("request failed ({status}): {text}").into());
    }
    println!("{text}");
    Ok(())
}
pub async fn status(endpoint: &str) -> Result<(), Box<dyn std::error::Error>> {
    request(reqwest::Method::GET, endpoint, None).await
}
pub fn server_base() -> String {
    crate::auth::current()
        .map(|(_, profile, _)| profile.server)
        .unwrap_or_else(|_| "http://127.0.0.1:8080".into())
}

pub async fn auth(command: AuthCommand) -> Result<(), Box<dyn std::error::Error>> {
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
pub fn profile(command: ProfileCommand) -> Result<(), Box<dyn std::error::Error>> {
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
