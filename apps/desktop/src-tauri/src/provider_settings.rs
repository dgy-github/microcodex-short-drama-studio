use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use story_provider::ProviderRoute;
use uuid::Uuid;

use crate::credentials::CredentialService;
use crate::CommandError;

const PROFILE: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRouteSettings {
    pub schema: String,
    pub provider: String,
    pub profile: String,
    pub endpoint: String,
    pub model: String,
    pub thinking_disabled: bool,
    pub source: String,
    pub record_id: Option<String>,
    pub updated_at_unix_ms: Option<u64>,
}

pub struct ProviderSettingsService {
    root: PathBuf,
    gate: Mutex<()>,
}

impl ProviderSettingsService {
    pub fn new(root: PathBuf) -> Result<Self, CommandError> {
        fs::create_dir_all(&root).map_err(|_| CommandError::provider_settings_unavailable())?;
        Ok(Self {
            root,
            gate: Mutex::new(()),
        })
    }

    pub fn load(
        &self,
        provider: &str,
        profile: &str,
    ) -> Result<ProviderRouteSettings, CommandError> {
        validate_identity(provider, profile)?;
        let _guard = self
            .gate
            .lock()
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        self.load_unlocked(provider)
    }

    pub fn save(
        &self,
        provider: &str,
        profile: &str,
        endpoint: String,
        model: String,
    ) -> Result<ProviderRouteSettings, CommandError> {
        validate_identity(provider, profile)?;
        ProviderRoute::validate(&endpoint, &model)
            .map_err(|_| CommandError::invalid_provider_route())?;
        let _guard = self
            .gate
            .lock()
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        let previous_timestamp = self
            .load_unlocked(provider)?
            .updated_at_unix_ms
            .unwrap_or_default();
        let updated_at_unix_ms = now_unix_ms()?.max(previous_timestamp.saturating_add(1));
        let record = ProviderRouteSettings {
            schema: "desktop-provider-route/v1".into(),
            provider: provider.into(),
            profile: PROFILE.into(),
            endpoint,
            model,
            thinking_disabled: provider == "aliyun_bailian",
            source: "user".into(),
            record_id: Some(format!("route_{}", Uuid::new_v4().simple())),
            updated_at_unix_ms: Some(updated_at_unix_ms),
        };
        self.write_record(&record)?;
        Ok(record)
    }

    pub fn route(
        &self,
        credentials: &CredentialService,
        provider: &str,
    ) -> Result<ProviderRoute, CommandError> {
        let settings = self.load(provider, PROFILE)?;
        let secret = credentials.load(provider, PROFILE)?;
        let mut route = ProviderRoute::new(settings.endpoint, settings.model, secret)
            .map_err(|_| CommandError::invalid_provider_route())?;
        if settings.thinking_disabled {
            route = route.with_thinking_disabled();
        }
        Ok(route)
    }

    fn load_unlocked(&self, provider: &str) -> Result<ProviderRouteSettings, CommandError> {
        let directory = self.root.join(provider);
        if !directory.exists() {
            return default_settings(provider);
        }
        let mut latest: Option<ProviderRouteSettings> = None;
        for entry in
            fs::read_dir(&directory).map_err(|_| CommandError::provider_settings_unavailable())?
        {
            let path = entry
                .map_err(|_| CommandError::provider_settings_unavailable())?
                .path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                return Err(CommandError::provider_settings_unavailable());
            };
            if name.ends_with(".partial") {
                continue;
            }
            if !name.starts_with("route_") || !name.ends_with(".json") {
                continue;
            }
            let bytes =
                fs::read(&path).map_err(|_| CommandError::provider_settings_unavailable())?;
            let candidate: ProviderRouteSettings = serde_json::from_slice(&bytes)
                .map_err(|_| CommandError::provider_settings_unavailable())?;
            validate_record(&candidate, provider)?;
            if latest
                .as_ref()
                .map(|current| record_key(&candidate) > record_key(current))
                .unwrap_or(true)
            {
                latest = Some(candidate);
            }
        }
        latest.map_or_else(|| default_settings(provider), Ok)
    }

    fn write_record(&self, record: &ProviderRouteSettings) -> Result<(), CommandError> {
        let directory = self.root.join(&record.provider);
        fs::create_dir_all(&directory)
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        let record_id = record
            .record_id
            .as_deref()
            .ok_or_else(CommandError::invalid_provider_route)?;
        let target = directory.join(format!("{record_id}.json"));
        let partial = directory.join(format!("{record_id}.json.partial"));
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&partial)
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            let _ = fs::remove_file(&partial);
            return Err(if error.kind() == std::io::ErrorKind::InvalidInput {
                CommandError::invalid_provider_route()
            } else {
                CommandError::provider_settings_unavailable()
            });
        }
        drop(file);
        if let Err(_error) = fs::rename(&partial, &target) {
            let _ = fs::remove_file(&partial);
            return Err(CommandError::provider_settings_unavailable());
        }
        Ok(())
    }
}

fn validate_identity(provider: &str, profile: &str) -> Result<(), CommandError> {
    if !matches!(provider, "deepseek" | "aliyun_bailian") || profile != PROFILE {
        return Err(CommandError::invalid_provider());
    }
    Ok(())
}

fn validate_record(record: &ProviderRouteSettings, provider: &str) -> Result<(), CommandError> {
    if record.schema != "desktop-provider-route/v1"
        || record.provider != provider
        || record.profile != PROFILE
        || record.source != "user"
        || record.thinking_disabled != (provider == "aliyun_bailian")
        || !record
            .record_id
            .as_deref()
            .map(valid_record_id)
            .unwrap_or(false)
        || record.updated_at_unix_ms.is_none()
    {
        return Err(CommandError::provider_settings_unavailable());
    }
    ProviderRoute::validate(&record.endpoint, &record.model)
        .map_err(|_| CommandError::provider_settings_unavailable())
}

fn valid_record_id(value: &str) -> bool {
    value.len() == 38
        && value.starts_with("route_")
        && value[6..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn record_key(record: &ProviderRouteSettings) -> (u64, &str) {
    (
        record.updated_at_unix_ms.unwrap_or_default(),
        record.record_id.as_deref().unwrap_or_default(),
    )
}

fn default_settings(provider: &str) -> Result<ProviderRouteSettings, CommandError> {
    let (endpoint, model, thinking_disabled) = match provider {
        "deepseek" => (
            "https://api.deepseek.com/chat/completions",
            "deepseek-v4-pro",
            false,
        ),
        "aliyun_bailian" => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "qwen3-vl-plus",
            true,
        ),
        _ => return Err(CommandError::invalid_provider()),
    };
    Ok(ProviderRouteSettings {
        schema: "desktop-provider-route/v1".into(),
        provider: provider.into(),
        profile: PROFILE.into(),
        endpoint: endpoint.into(),
        model: model.into(),
        thinking_disabled,
        source: "default".into(),
        record_id: None,
        updated_at_unix_ms: None,
    })
}

fn now_unix_ms() -> Result<u64, CommandError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::provider_settings_unavailable())?
        .as_millis();
    u64::try_from(millis).map_err(|_| CommandError::provider_settings_unavailable())
}

pub fn default_provider_settings_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("MicrocodeX")
        .join("ShortDramaStudio")
        .join("provider-routes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn defaults_and_invalid_routes_are_fail_closed() {
        let directory = tempfile::tempdir().unwrap();
        let service = ProviderSettingsService::new(directory.path().into()).unwrap();
        let deepseek = service.load("deepseek", PROFILE).unwrap();
        assert_eq!(deepseek.source, "default");
        assert!(!deepseek.thinking_disabled);
        let bailian = service.load("aliyun_bailian", PROFILE).unwrap();
        assert!(bailian.thinking_disabled);
        assert!(service
            .save(
                "deepseek",
                PROFILE,
                "http://unsafe.test/chat/completions".into(),
                "model".into()
            )
            .is_err());
        assert!(service
            .save(
                "deepseek",
                PROFILE,
                "https://safe.test/chat/completions".into(),
                " ".into()
            )
            .is_err());
        assert!(service.load("unknown", PROFILE).is_err());
    }

    #[test]
    fn latest_immutable_record_wins_and_partial_is_ignored() {
        let directory = tempfile::tempdir().unwrap();
        let service = ProviderSettingsService::new(directory.path().into()).unwrap();
        let first = service
            .save(
                "deepseek",
                PROFILE,
                "https://one.test/chat/completions".into(),
                "one".into(),
            )
            .unwrap();
        let second = service
            .save(
                "deepseek",
                PROFILE,
                "https://two.test/chat/completions".into(),
                "two".into(),
            )
            .unwrap();
        let partial = directory
            .path()
            .join("deepseek")
            .join("route_deadbeef.json.partial");
        fs::write(partial, b"{").unwrap();
        let loaded = service.load("deepseek", PROFILE).unwrap();
        assert_eq!(loaded.record_id, second.record_id);
        assert_ne!(loaded.record_id, first.record_id);
        assert_eq!(loaded.model, "two");
    }

    #[test]
    fn route_projection_matches_json_contract() {
        let directory = tempfile::tempdir().unwrap();
        let service = ProviderSettingsService::new(directory.path().into()).unwrap();
        let route = serde_json::to_value(service.load("deepseek", PROFILE).unwrap()).unwrap();
        let schema: serde_json::Value = serde_json::from_slice(
            &fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../schemas/desktop-provider-route-v1.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&route));
    }
}
