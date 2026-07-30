use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use story_provider::{
    ProviderCredentialId, ProviderCredentialStore, ProviderSecret, WindowsCredentialStore,
};

use crate::CommandError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CredentialStatus {
    pub schema: &'static str,
    pub provider: String,
    pub profile: String,
    pub configured: bool,
}

pub struct CredentialService {
    store: WindowsCredentialStore,
    audit_path: PathBuf,
}

impl CredentialService {
    pub fn new() -> Self {
        Self {
            store: WindowsCredentialStore::new(),
            audit_path: default_audit_path(),
        }
    }

    pub fn status(&self, provider: &str, profile: &str) -> Result<CredentialStatus, CommandError> {
        let id = credential_id(provider, profile)?;
        let configured = self
            .store
            .get(&id)
            .map_err(|_| CommandError::credential_unavailable())?
            .is_some();
        Ok(status_value(provider, profile, configured))
    }

    pub fn store(
        &self,
        provider: &str,
        profile: &str,
        secret: String,
    ) -> Result<CredentialStatus, CommandError> {
        let id = credential_id(provider, profile)?;
        let existed = self
            .store
            .get(&id)
            .map_err(|_| CommandError::credential_unavailable())?
            .is_some();
        if secret.trim().is_empty() || secret.len() > 4096 {
            return Err(CommandError::invalid_secret());
        }
        let secret =
            ProviderSecret::new(secret.into_bytes()).map_err(|_| CommandError::invalid_secret())?;
        self.store
            .set(&id, secret.expose_secret())
            .map_err(|_| CommandError::credential_unavailable())?;
        append_audit_event(
            &self.audit_path,
            provider,
            profile,
            if existed { "rotated" } else { "configured" },
        )?;
        Ok(status_value(provider, profile, true))
    }

    pub fn delete(&self, provider: &str, profile: &str) -> Result<CredentialStatus, CommandError> {
        let id = credential_id(provider, profile)?;
        self.store
            .delete(&id)
            .map_err(|_| CommandError::credential_unavailable())?;
        append_audit_event(&self.audit_path, provider, profile, "deleted")?;
        Ok(status_value(provider, profile, false))
    }

    pub fn load(&self, provider: &str, profile: &str) -> Result<ProviderSecret, CommandError> {
        let id = credential_id(provider, profile)?;
        self.store
            .get(&id)
            .map_err(|_| CommandError::credential_unavailable())?
            .ok_or_else(CommandError::credential_missing)
    }

    pub fn audit_events(&self) -> Result<Vec<CredentialAuditEvent>, CommandError> {
        read_audit_events(&self.audit_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAuditEvent {
    pub schema: String,
    pub sequence: u64,
    pub occurred_at_unix_seconds: u64,
    pub provider: String,
    pub profile: String,
    pub action: String,
    pub previous_hash: String,
    pub event_hash: String,
}

fn append_audit_event(
    path: &Path,
    provider: &str,
    profile: &str,
    action: &str,
) -> Result<(), CommandError> {
    let mut events = read_audit_events(path)?;
    let sequence = events.last().map_or(1, |event| event.sequence + 1);
    let previous_hash = events
        .last()
        .map_or_else(|| "genesis".to_owned(), |event| event.event_hash.clone());
    let occurred_at_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::credential_audit_unavailable())?
        .as_secs();
    let material = format!(
        "{sequence}|{occurred_at_unix_seconds}|{provider}|{profile}|{action}|{previous_hash}"
    );
    let event_hash = format!("{:x}", Sha256::digest(material.as_bytes()));
    events.push(CredentialAuditEvent {
        schema: "credential-audit-event/v1".into(),
        sequence,
        occurred_at_unix_seconds,
        provider: provider.into(),
        profile: profile.into(),
        action: action.into(),
        previous_hash,
        event_hash,
    });
    let parent = path
        .parent()
        .ok_or_else(CommandError::credential_audit_unavailable)?;
    std::fs::create_dir_all(parent).map_err(|_| CommandError::credential_audit_unavailable())?;
    let temporary = path.with_extension("partial.json");
    let bytes = serde_json::to_vec_pretty(&events)
        .map_err(|_| CommandError::credential_audit_unavailable())?;
    std::fs::write(&temporary, bytes).map_err(|_| CommandError::credential_audit_unavailable())?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|_| CommandError::credential_audit_unavailable())?;
    }
    std::fs::rename(temporary, path).map_err(|_| CommandError::credential_audit_unavailable())
}

fn read_audit_events(path: &Path) -> Result<Vec<CredentialAuditEvent>, CommandError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(path).map_err(|_| CommandError::credential_audit_unavailable())?;
    let events: Vec<CredentialAuditEvent> =
        serde_json::from_slice(&bytes).map_err(|_| CommandError::credential_audit_unavailable())?;
    let mut previous_hash = "genesis".to_owned();
    for (index, event) in events.iter().enumerate() {
        let material = format!(
            "{}|{}|{}|{}|{}|{}",
            event.sequence,
            event.occurred_at_unix_seconds,
            event.provider,
            event.profile,
            event.action,
            event.previous_hash
        );
        let expected = format!("{:x}", Sha256::digest(material.as_bytes()));
        if event.schema != "credential-audit-event/v1"
            || event.sequence != index as u64 + 1
            || event.previous_hash != previous_hash
            || event.event_hash != expected
        {
            return Err(CommandError::credential_audit_unavailable());
        }
        previous_hash.clone_from(&event.event_hash);
    }
    Ok(events)
}

fn default_audit_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("MicrocodeX")
        .join("ShortDramaStudio")
        .join("credential-audit.json")
}

fn credential_id(provider: &str, profile: &str) -> Result<ProviderCredentialId, CommandError> {
    if !matches!(provider, "deepseek" | "aliyun_bailian") || profile != "default" {
        return Err(CommandError::invalid_provider());
    }
    ProviderCredentialId::new(provider, profile).map_err(|_| CommandError::invalid_provider())
}

fn status_value(provider: &str, profile: &str, configured: bool) -> CredentialStatus {
    CredentialStatus {
        schema: "desktop-credential-status/v1",
        provider: provider.to_owned(),
        profile: profile.to_owned(),
        configured,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_status_serializes_without_secret_material() {
        let encoded = serde_json::to_string(&status_value("deepseek", "default", true)).unwrap();
        assert_eq!(
            encoded,
            r#"{"schema":"desktop-credential-status/v1","provider":"deepseek","profile":"default","configured":true}"#
        );
        assert!(!encoded.contains("secret"));
        assert!(credential_id("unknown", "default").is_err());
    }

    #[test]
    fn credential_audit_is_hash_chained_and_contains_no_secret() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audit.json");
        append_audit_event(&path, "deepseek", "default", "configured").unwrap();
        append_audit_event(&path, "deepseek", "default", "rotated").unwrap();
        let events = read_audit_events(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].previous_hash, events[0].event_hash);
        let encoded = std::fs::read_to_string(path).unwrap();
        assert!(!encoded.contains("sk-"));
        assert!(!encoded.contains("secret"));
    }
}
