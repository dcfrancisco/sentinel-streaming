use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArtifactType {
    Snapshot,
    Clip,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArtifactOutcome {
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetentionMetadata {
    pub retention_class: String,
    pub expires_at: Option<u128>,
    pub protected_from_auto_delete: bool,
}

impl Default for RetentionMetadata {
    fn default() -> Self {
        Self {
            retention_class: "diagnostic".into(),
            expires_at: None,
            protected_from_auto_delete: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaArtifact {
    pub artifact_id: String,
    pub source_id: String,
    pub artifact_type: ArtifactType,
    pub created_at: u128,
    pub duration_ms: Option<u64>,
    pub content_type: String,
    pub codec: Option<String>,
    pub audio_present: Option<bool>,
    pub audio_codec: Option<String>,
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u16>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size_bytes: u64,
    pub storage_reference: String,
    pub checksum_sha256: String,
    pub requested_by: String,
    pub correlation_id: String,
    pub capture_mechanism: String,
    pub retention: RetentionMetadata,
    pub outcome: ArtifactOutcome,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum ArtifactStoreError {
    #[error("artifact was not found")]
    NotFound,
    #[error("artifact storage is unavailable: {0}")]
    Unavailable(String),
    #[error("artifact storage write failed: {0}")]
    WriteFailed(String),
    #[error("artifact path is invalid")]
    InvalidPath,
}

#[async_trait]
pub trait MediaArtifactStore: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    async fn store(
        &self,
        source_id: &str,
        artifact_type: ArtifactType,
        content_type: &str,
        bytes: Vec<u8>,
        duration_ms: Option<u64>,
        codec: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
        audio_present: Option<bool>,
        audio_codec: Option<String>,
        audio_sample_rate: Option<u32>,
        audio_channels: Option<u16>,
        requested_by: String,
        correlation_id: String,
        capture_mechanism: String,
    ) -> Result<MediaArtifact, ArtifactStoreError>;
    async fn metadata(&self, artifact_id: &str) -> Result<MediaArtifact, ArtifactStoreError>;
    async fn read(&self, artifact_id: &str)
        -> Result<(MediaArtifact, Vec<u8>), ArtifactStoreError>;
    async fn delete(&self, artifact_id: &str) -> Result<MediaArtifact, ArtifactStoreError>;
}

#[derive(Clone)]
pub struct FilesystemArtifactStore {
    root: Arc<PathBuf>,
    metadata: Arc<RwLock<HashMap<String, MediaArtifact>>>,
    next_id: Arc<AtomicU64>,
}

impl FilesystemArtifactStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Arc::new(root.into()),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn path_for(&self, artifact_id: &str) -> Result<PathBuf, ArtifactStoreError> {
        if artifact_id.is_empty()
            || !artifact_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ArtifactStoreError::InvalidPath);
        }
        Ok(self.root.join(format!("{artifact_id}.bin")))
    }

    fn artifact_id(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!(
            "artifact-{now}-{}",
            self.next_id.fetch_add(1, Ordering::Relaxed)
        )
    }
}

#[async_trait]
impl MediaArtifactStore for FilesystemArtifactStore {
    #[allow(clippy::too_many_arguments)]
    async fn store(
        &self,
        source_id: &str,
        artifact_type: ArtifactType,
        content_type: &str,
        bytes: Vec<u8>,
        duration_ms: Option<u64>,
        codec: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
        audio_present: Option<bool>,
        audio_codec: Option<String>,
        audio_sample_rate: Option<u32>,
        audio_channels: Option<u16>,
        requested_by: String,
        correlation_id: String,
        capture_mechanism: String,
    ) -> Result<MediaArtifact, ArtifactStoreError> {
        let root = self.root.as_ref();
        tokio::fs::create_dir_all(root)
            .await
            .map_err(|e| ArtifactStoreError::Unavailable(e.to_string()))?;
        let artifact_id = self.artifact_id();
        let path = self.path_for(&artifact_id)?;
        let temporary = root.join(format!(".{artifact_id}.tmp"));
        tokio::fs::write(&temporary, &bytes)
            .await
            .map_err(|e| ArtifactStoreError::WriteFailed(e.to_string()))?;
        tokio::fs::rename(&temporary, &path)
            .await
            .map_err(|e| ArtifactStoreError::WriteFailed(e.to_string()))?;
        let mut digest = Sha256::new();
        digest.update(&bytes);
        let artifact = MediaArtifact {
            artifact_id: artifact_id.clone(),
            source_id: source_id.into(),
            artifact_type,
            created_at: crate::events::now_ms(),
            duration_ms,
            content_type: content_type.into(),
            codec,
            audio_present,
            audio_codec,
            audio_sample_rate,
            audio_channels,
            width,
            height,
            size_bytes: bytes.len() as u64,
            storage_reference: format!("artifacts/{artifact_id}"),
            checksum_sha256: format!("{:x}", digest.finalize()),
            requested_by,
            correlation_id,
            capture_mechanism,
            retention: RetentionMetadata::default(),
            outcome: ArtifactOutcome::Complete,
        };
        self.metadata
            .write()
            .await
            .insert(artifact_id, artifact.clone());
        Ok(artifact)
    }

    async fn metadata(&self, artifact_id: &str) -> Result<MediaArtifact, ArtifactStoreError> {
        self.metadata
            .read()
            .await
            .get(artifact_id)
            .cloned()
            .ok_or(ArtifactStoreError::NotFound)
    }

    async fn read(
        &self,
        artifact_id: &str,
    ) -> Result<(MediaArtifact, Vec<u8>), ArtifactStoreError> {
        let artifact = self.metadata(artifact_id).await?;
        let path = self.path_for(artifact_id)?;
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| ArtifactStoreError::Unavailable(e.to_string()))?;
        Ok((artifact, bytes))
    }

    async fn delete(&self, artifact_id: &str) -> Result<MediaArtifact, ArtifactStoreError> {
        let artifact = self.metadata(artifact_id).await?;
        let path = self.path_for(artifact_id)?;
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| ArtifactStoreError::WriteFailed(e.to_string()))?;
        self.metadata.write().await.remove(artifact_id);
        Ok(artifact)
    }
}

pub fn default_root() -> PathBuf {
    std::env::var("SENTINEL_MEDIA_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("sentinel-artifacts"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> FilesystemArtifactStore {
        FilesystemArtifactStore::new(std::env::temp_dir().join(format!(
            "sentinel-artifacts-test-{}",
            crate::events::now_ms()
        )))
    }

    #[tokio::test]
    async fn stores_metadata_and_content_without_exposing_filesystem_path() {
        let store = store();
        let artifact = store
            .store(
                "camera-1",
                ArtifactType::Snapshot,
                "image/jpeg",
                b"jpeg".to_vec(),
                None,
                None,
                Some(640),
                Some(360),
                Some(false),
                None,
                None,
                None,
                "operator".into(),
                "request-1".into(),
                "decoded_frame_buffer".into(),
            )
            .await
            .unwrap();
        let json = serde_json::to_string(&artifact).unwrap();
        assert!(json.contains("request-1"));
        assert!(json.contains("artifacts/"));
        assert!(!json.contains("sentinel-artifacts-test-"));
        assert_eq!(store.read(&artifact.artifact_id).await.unwrap().1, b"jpeg");
        assert_eq!(artifact.size_bytes, 4);
        assert_eq!(artifact.checksum_sha256.len(), 64);
    }

    #[tokio::test]
    async fn rejects_path_traversal_and_deletes_artifact() {
        let store = store();
        assert!(matches!(
            store.metadata("../secret").await,
            Err(ArtifactStoreError::NotFound)
        ));
        let artifact = store
            .store(
                "camera-1",
                ArtifactType::Clip,
                "video/mp4",
                vec![1, 2],
                Some(1000),
                None,
                None,
                None,
                Some(false),
                None,
                None,
                None,
                "operator".into(),
                "corr".into(),
                "ffmpeg_rtsp_copy".into(),
            )
            .await
            .unwrap();
        assert_eq!(
            store
                .delete(&artifact.artifact_id)
                .await
                .unwrap()
                .artifact_id,
            artifact.artifact_id
        );
        assert!(matches!(
            store.metadata(&artifact.artifact_id).await,
            Err(ArtifactStoreError::NotFound)
        ));
    }
}
