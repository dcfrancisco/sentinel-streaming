use crate::{api::AppState, events::EventRecord};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: &str = "support-bundle.v1";

pub async fn snapshot(state: &AppState, request_id: Option<&str>) -> Value {
    let config = sanitized_config(state);
    let config_hash = sha256_json(&config);
    let runtime = state.runtime.snapshot().await;
    let event_records = state.events.store().recent(100).await;
    let operational_events: Vec<Value> = event_records
        .into_iter()
        .filter(is_operational_event)
        .map(|event| sanitize_value(serde_json::to_value(event).unwrap_or(Value::Null)))
        .collect();
    let gateway = state.sources.media_gateway_health().await;
    let ready = state
        .health
        .ready
        .load(std::sync::atomic::Ordering::Relaxed);
    let request_id = request_id
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("support-{}", crate::events::now_ms()));

    serde_json::json!({
        "manifest": {
            "schemaVersion": SCHEMA_VERSION,
            "generatedAt": crate::events::now_ms(),
            "instanceId": state.config.instance_id,
            "deploymentProfile": state.config.deployment_profile,
            "runtimeMode": state.config.security.mode.as_str(),
            "configHash": config_hash,
            "apiVersion": "v1",
            "requestId": request_id.clone(),
            "correlationId": request_id,
            "causationId": Value::Null
        },
        "version": {
            "name": "sentinel-streaming",
            "version": env!("CARGO_PKG_VERSION"),
            "apiVersion": "v1",
            "schemaVersion": SCHEMA_VERSION,
            "instanceId": state.config.instance_id,
            "deploymentProfile": state.config.deployment_profile,
            "runtimeMode": state.config.security.mode.as_str()
        },
        "health": {
            "live": true,
            "ready": ready,
            "runtime": runtime,
            "components": state.health_monitor.snapshot().await,
            "metrics": state.metrics.snapshot()
        },
        "sanitizedConfig": config,
        "sourceSummary": state.sources.list().await,
        "recentOperationalEvents": operational_events,
        "dependencyHealth": {
            "mediaGateway": gateway,
            "ffmpegConfigured": std::env::var_os("SENTINEL_FFMPEG").is_some()
        }
    })
}

fn sanitized_config(state: &AppState) -> Value {
    sanitize_value(serde_json::to_value(&state.config).unwrap_or(Value::Null))
}

fn sha256_json(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    format!("sha256:{digest:x}")
}

fn is_operational_event(event: &EventRecord) -> bool {
    let event_type = event.event_type.to_ascii_lowercase();
    !event_type.starts_with("vision.") && event_type != "scene.observed"
}

pub fn sanitize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut sanitized = Map::new();
            for (key, value) in object {
                let normalized = key.to_ascii_lowercase();
                if [
                    "password",
                    "secret",
                    "token",
                    "authorization",
                    "credential",
                    "api_key",
                    "apikey",
                ]
                .iter()
                .any(|term| normalized.contains(term))
                {
                    sanitized.insert(key, Value::String("[REDACTED]".into()));
                } else {
                    sanitized.insert(key, sanitize_value(value));
                }
            }
            Value::Object(sanitized)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sanitize_value).collect()),
        Value::String(value) => Value::String(sanitize_string(&value)),
        other => other,
    }
}

fn sanitize_string(value: &str) -> String {
    url::Url::parse(value)
        .map(|mut parsed| {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                let _ = parsed.set_username("[REDACTED]");
                let _ = parsed.set_password(Some("[REDACTED]"));
            }
            parsed.to_string()
        })
        .unwrap_or_else(|_| value.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_secret_keys_and_embedded_url_credentials() {
        let value = sanitize_value(serde_json::json!({
            "token": "do-not-export",
            "nested": {"password": "hidden"},
            "url": "rtsp://user:pass@example.test/stream"
        }));
        assert_eq!(value["token"], "[REDACTED]");
        assert_eq!(value["nested"]["password"], "[REDACTED]");
        assert!(value["url"].as_str().unwrap().contains("REDACTED"));
        assert!(!value.to_string().contains("do-not-export"));
    }
}
