use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::PathBuf,
    sync::Arc,
};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Authority {
    ViewStream,
    ViewSource,
    ManageSource,
    RunOnboarding,
    ControlPtz,
    ViewDiagnostics,
    AdministerSystem,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Role {
    Viewer,
    Operator,
    Administrator,
}

impl Role {
    pub fn authorities(&self) -> &'static [Authority] {
        match self {
            Self::Viewer => &[Authority::ViewStream, Authority::ViewSource],
            Self::Operator => &[
                Authority::ViewStream,
                Authority::ViewSource,
                Authority::ControlPtz,
                Authority::ViewDiagnostics,
            ],
            Self::Administrator => &[
                Authority::ViewStream,
                Authority::ViewSource,
                Authority::ManageSource,
                Authority::RunOnboarding,
                Authority::ControlPtz,
                Authority::ViewDiagnostics,
                Authority::AdministerSystem,
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Principal {
    pub id: String,
    pub role: Role,
}

impl Principal {
    pub fn allows(&self, authority: Authority) -> bool {
        self.role.authorities().contains(&authority)
    }
}

pub trait Authenticator: Send + Sync {
    fn authenticate(&self, token: Option<&str>) -> Option<Principal>;
    fn configured(&self) -> bool;
}

#[derive(Clone, Default)]
pub struct PtzAuthority;

impl PtzAuthority {
    /// Explicit service boundary for consequential camera control. Deployments
    /// can replace this seam with actor- and policy-aware authorization without
    /// exposing ONVIF access to clients.
    pub fn authorize(&self, principal: Option<&Principal>) -> Result<String, &'static str> {
        let principal = principal.ok_or("PTZ control requires authenticated operator authority")?;
        if principal.allows(Authority::ControlPtz) {
            Ok(principal.id.clone())
        } else {
            Err("PTZ control requires CONTROL_PTZ authority")
        }
    }
}
#[derive(Clone)]
pub struct BearerAuthenticator {
    tokens: Arc<HashMap<String, Principal>>,
}
impl BearerAuthenticator {
    pub fn from_env() -> Self {
        let mut tokens = HashMap::new();
        if let Ok(token) = std::env::var("SENTINEL_VIEWER_TOKEN") {
            tokens.insert(
                token,
                Principal {
                    id: "viewer".into(),
                    role: Role::Viewer,
                },
            );
        }
        if let Ok(token) = std::env::var("SENTINEL_OPERATOR_TOKEN") {
            tokens.insert(
                token,
                Principal {
                    id: "operator".into(),
                    role: Role::Operator,
                },
            );
        }
        if let Ok(token) = std::env::var("SENTINEL_ADMIN_TOKEN") {
            tokens.insert(
                token,
                Principal {
                    id: "administrator".into(),
                    role: Role::Administrator,
                },
            );
        }
        // First-run bootstrap is deliberately an explicitly supplied,
        // temporary administrator token. There is no built-in default.
        if let Ok(token) = std::env::var("SENTINEL_BOOTSTRAP_TOKEN") {
            tokens.entry(token).or_insert_with(|| Principal {
                id: "bootstrap-administrator".into(),
                role: Role::Administrator,
            });
        }
        // Preserve the pre-SS-WP-009 deployment contract. The legacy token is
        // intentionally an operator token, never an administrator token.
        if let Ok(token) = std::env::var("SENTINEL_API_TOKEN") {
            tokens.entry(token).or_insert_with(|| Principal {
                id: "operator".into(),
                role: Role::Operator,
            });
        }
        Self {
            tokens: Arc::new(tokens),
        }
    }
    pub fn enabled(&self) -> bool {
        self.configured()
    }
    pub fn principal(&self, token: Option<&str>) -> Option<Principal> {
        self.authenticate(token)
    }

    #[cfg(test)]
    fn from_test_tokens(tokens: &[(&str, Principal)]) -> Self {
        Self {
            tokens: Arc::new(
                tokens
                    .iter()
                    .map(|(token, principal)| ((*token).into(), principal.clone()))
                    .collect(),
            ),
        }
    }
}
impl Authenticator for BearerAuthenticator {
    fn authenticate(&self, token: Option<&str>) -> Option<Principal> {
        token.and_then(|token| self.tokens.get(token).cloned())
    }
    fn configured(&self) -> bool {
        !self.tokens.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_authorities_are_explicit_and_cumulative() {
        let viewer = Principal {
            id: "v".into(),
            role: Role::Viewer,
        };
        let operator = Principal {
            id: "o".into(),
            role: Role::Operator,
        };
        let admin = Principal {
            id: "a".into(),
            role: Role::Administrator,
        };
        assert!(viewer.allows(Authority::ViewSource));
        assert!(!viewer.allows(Authority::ControlPtz));
        assert!(operator.allows(Authority::ControlPtz));
        assert!(!operator.allows(Authority::ManageSource));
        assert!(admin.allows(Authority::RunOnboarding));
    }

    #[test]
    fn ptz_authority_rejects_missing_or_insufficient_principal() {
        let authority = PtzAuthority;
        assert!(authority.authorize(None).is_err());
        let viewer = Principal {
            id: "v".into(),
            role: Role::Viewer,
        };
        assert!(authority.authorize(Some(&viewer)).is_err());
        let operator = Principal {
            id: "o".into(),
            role: Role::Operator,
        };
        assert_eq!(authority.authorize(Some(&operator)).unwrap(), "o");
    }

    #[test]
    fn bearer_authentication_returns_role_and_rejects_invalid_token() {
        let auth = BearerAuthenticator::from_test_tokens(&[(
            "operator-token",
            Principal {
                id: "operator-1".into(),
                role: Role::Operator,
            },
        )]);
        assert_eq!(auth.authenticate(Some("bad")), None);
        let principal = auth.authenticate(Some("operator-token")).unwrap();
        assert_eq!(principal.id, "operator-1");
        assert!(principal.allows(Authority::ControlPtz));
    }
}
