use crate::onvif::{CameraCapabilities, MediaProfile, OnvifDevice, OnvifInspection};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct OnboardingInspectRequest {
    pub endpoint: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OnboardingCompleteRequest {
    pub source_id: String,
    pub name: String,
    pub location: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingCheck {
    pub id: String,
    pub state: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingFailure {
    pub stage: String,
    pub code: String,
    pub message: String,
    pub technical_detail: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OnboardingState {
    Discovered,
    Inspected,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingSessionView {
    pub session_id: String,
    pub state: OnboardingState,
    pub devices: Vec<OnvifDevice>,
    pub selected_device: Option<OnvifDevice>,
    pub selected_profile: Option<MediaProfile>,
    pub capabilities: Option<CameraCapabilities>,
    pub checks: Vec<OnboardingCheck>,
    pub failure: Option<OnboardingFailure>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OnboardingCompletion {
    pub success: bool,
    pub state: OnboardingState,
    pub source_id: Option<String>,
    pub checks: Vec<OnboardingCheck>,
    pub failure: Option<OnboardingFailure>,
}

#[derive(Clone)]
pub(crate) struct OnboardingDraft {
    pub session_id: String,
    pub devices: Vec<OnvifDevice>,
    pub endpoint: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub inspection: Option<OnvifInspection>,
    pub selected_profile: Option<MediaProfile>,
}

impl OnboardingDraft {
    pub fn discovered(session_id: String, devices: Vec<OnvifDevice>) -> Self {
        Self {
            session_id,
            devices,
            endpoint: None,
            username: None,
            password: None,
            inspection: None,
            selected_profile: None,
        }
    }

    pub fn view(&self) -> OnboardingSessionView {
        let (selected_device, capabilities) = self
            .inspection
            .as_ref()
            .map(|inspection| {
                (
                    Some(inspection.device.clone()),
                    Some(inspection.device.capabilities.clone()),
                )
            })
            .unwrap_or((None, None));
        let state = if self.inspection.is_some() {
            OnboardingState::Inspected
        } else {
            OnboardingState::Discovered
        };
        let checks = if let Some(capabilities) = &capabilities {
            vec![
                check("onvif", "pass", "Camera capabilities discovered."),
                check(
                    "video_profile",
                    if self.selected_profile.is_some() {
                        "pass"
                    } else {
                        "fail"
                    },
                    if self.selected_profile.is_some() {
                        "A usable video profile was selected automatically."
                    } else {
                        "Camera is reachable, but no usable video profile was found."
                    },
                ),
                check(
                    "audio",
                    if capabilities.audio {
                        "supported"
                    } else {
                        "unsupported"
                    },
                    if capabilities.audio {
                        "Audio is supported by the inspected profile set."
                    } else {
                        "Audio was not advertised by the inspected profiles."
                    },
                ),
                check(
                    "ptz",
                    if capabilities.ptz.supported {
                        "supported"
                    } else {
                        "unsupported"
                    },
                    if capabilities.ptz.supported {
                        "PTZ is supported and can be tested after setup."
                    } else {
                        "PTZ is not advertised by this camera."
                    },
                ),
            ]
        } else {
            vec![check(
                "onvif",
                "not_checked",
                "ONVIF inspection has not run yet.",
            )]
        };
        OnboardingSessionView {
            session_id: self.session_id.clone(),
            state,
            devices: self.devices.clone(),
            selected_device,
            selected_profile: self.selected_profile.clone(),
            capabilities,
            checks,
            failure: None,
        }
    }
}

pub(crate) fn check(id: &str, state: &str, message: &str) -> OnboardingCheck {
    OnboardingCheck {
        id: id.into(),
        state: state.into(),
        message: message.into(),
    }
}
