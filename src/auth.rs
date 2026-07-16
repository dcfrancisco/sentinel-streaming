use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::PathBuf, sync::Arc};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub server: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ProfileFile {
    current: Option<String>,
    profiles: BTreeMap<String, Profile>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sentinel-streaming")
        .join("profiles.json")
}
fn load() -> io::Result<ProfileFile> {
    let path = config_path();
    if !path.exists() {
        return Ok(ProfileFile::default());
    }
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}
fn save(data: &ProfileFile) -> io::Result<()> {
    let path = config_path();
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(
        &path,
        serde_json::to_vec_pretty(data).map_err(io::Error::other)?,
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
fn credential(profile: &str) -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new("sentinel-streaming", profile)
}

pub fn login(profile: &str, server: &str, token: &str) -> anyhow::Result<()> {
    let mut data = load()?;
    data.profiles.insert(
        profile.into(),
        Profile {
            server: server.into(),
        },
    );
    data.current = Some(profile.into());
    save(&data)?;
    credential(profile)?.set_password(token)?;
    Ok(())
}
pub fn add_profile(profile: &str, server: &str) -> io::Result<()> {
    let mut data = load()?;
    data.profiles.insert(
        profile.into(),
        Profile {
            server: server.into(),
        },
    );
    if data.current.is_none() {
        data.current = Some(profile.into());
    }
    save(&data)
}
pub fn logout(profile: Option<&str>) -> anyhow::Result<()> {
    let mut data = load()?;
    let name = profile.map(str::to_owned).or(data.current.clone());
    if let Some(name) = name {
        data.profiles.remove(&name);
        let _ = credential(&name)?.delete_credential();
        if data.current.as_deref() == Some(&name) {
            data.current = data.profiles.keys().next().cloned();
        }
        save(&data)?;
    }
    Ok(())
}
pub fn list() -> io::Result<(Option<String>, BTreeMap<String, Profile>)> {
    let data = load()?;
    Ok((data.current, data.profiles))
}
pub fn use_profile(profile: &str) -> io::Result<()> {
    let mut data = load()?;
    if !data.profiles.contains_key(profile) {
        return Err(io::Error::new(io::ErrorKind::NotFound, "profile not found"));
    }
    data.current = Some(profile.into());
    save(&data)
}
pub fn current() -> anyhow::Result<(String, Profile, String)> {
    let data = load()?;
    let name = data
        .current
        .ok_or_else(|| anyhow::anyhow!("no active profile; run auth login"))?;
    let profile = data
        .profiles
        .get(&name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("active profile is missing"))?;
    let token = credential(&name)?.get_password()?;
    Ok((name, profile, token))
}

pub trait Authenticator: Send + Sync {
    fn authenticate(&self, token: Option<&str>) -> bool;
    fn principal(&self) -> &'static str;
}
#[derive(Clone)]
pub struct BearerAuthenticator {
    expected: Option<Arc<String>>,
}
impl BearerAuthenticator {
    pub fn from_env() -> Self {
        Self {
            expected: std::env::var("SENTINEL_API_TOKEN").ok().map(Arc::new),
        }
    }
    pub fn enabled(&self) -> bool {
        self.expected.is_some()
    }
}
impl Authenticator for BearerAuthenticator {
    fn authenticate(&self, token: Option<&str>) -> bool {
        match &self.expected {
            Some(expected) => token == Some(expected.as_str()),
            None => false,
        }
    }
    fn principal(&self) -> &'static str {
        "operator"
    }
}
