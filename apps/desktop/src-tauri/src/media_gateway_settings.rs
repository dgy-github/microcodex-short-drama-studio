use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use story_provider::MediaGatewayRoute;

use crate::CommandError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaGatewaySettings {
    pub schema: String,
    pub endpoint: String,
    #[serde(default)]
    pub coarse_endpoint: Option<String>,
    #[serde(default)]
    pub fine_endpoint: Option<String>,
}

pub struct MediaGatewaySettingsService {
    path: PathBuf,
}

impl MediaGatewaySettingsService {
    pub fn new(root: PathBuf) -> Result<Self, CommandError> {
        std::fs::create_dir_all(&root)
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        Ok(Self {
            path: root.join("media-gateway.json"),
        })
    }

    pub fn load(&self) -> Result<Option<MediaGatewaySettings>, CommandError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let value: MediaGatewaySettings = serde_json::from_slice(
            &std::fs::read(&self.path)
                .map_err(|_| CommandError::provider_settings_unavailable())?,
        )
        .map_err(|_| CommandError::provider_settings_unavailable())?;
        validate(&value).map_err(|_| CommandError::provider_settings_unavailable())?;
        Ok(Some(value))
    }

    pub fn save(&self, endpoint: String) -> Result<MediaGatewaySettings, CommandError> {
        MediaGatewayRoute::validate(&endpoint)
            .map_err(|_| CommandError::invalid_provider_route())?;
        let value = MediaGatewaySettings {
            schema: "desktop-media-gateway-settings/v1".into(),
            endpoint,
            coarse_endpoint: None,
            fine_endpoint: None,
        };
        let partial = self.path.with_extension("json.partial");
        std::fs::write(
            &partial,
            serde_json::to_vec_pretty(&value)
                .map_err(|_| CommandError::provider_settings_unavailable())?,
        )
        .map_err(|_| CommandError::provider_settings_unavailable())?;
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|_| CommandError::provider_settings_unavailable())?;
        }
        std::fs::rename(partial, &self.path)
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        Ok(value)
    }

    pub fn save_routes(&self, coarse_endpoint: String, fine_endpoint: String)
        -> Result<MediaGatewaySettings, CommandError> {
        MediaGatewayRoute::validate(&coarse_endpoint).map_err(|_| CommandError::invalid_provider_route())?;
        MediaGatewayRoute::validate(&fine_endpoint).map_err(|_| CommandError::invalid_provider_route())?;
        let value = MediaGatewaySettings { schema: "desktop-media-gateway-settings/v1".into(),
            endpoint: coarse_endpoint.clone(), coarse_endpoint: Some(coarse_endpoint),
            fine_endpoint: Some(fine_endpoint) };
        let partial = self.path.with_extension("json.partial");
        std::fs::write(&partial, serde_json::to_vec_pretty(&value).map_err(|_| CommandError::provider_settings_unavailable())?)
            .map_err(|_| CommandError::provider_settings_unavailable())?;
        if self.path.exists() { std::fs::remove_file(&self.path).map_err(|_| CommandError::provider_settings_unavailable())?; }
        std::fs::rename(partial, &self.path).map_err(|_| CommandError::provider_settings_unavailable())?;
        Ok(value)
    }
}

fn validate(value: &MediaGatewaySettings) -> Result<(), ()> {
    if value.schema != "desktop-media-gateway-settings/v1"
        || MediaGatewayRoute::validate(&value.endpoint).is_err()
        || value.coarse_endpoint.as_deref().is_some_and(|v| MediaGatewayRoute::validate(v).is_err())
        || value.fine_endpoint.as_deref().is_some_and(|v| MediaGatewayRoute::validate(v).is_err())
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_persist_without_credentials_and_reject_unsafe_routes() {
        let directory = tempfile::tempdir().unwrap();
        let service = MediaGatewaySettingsService::new(directory.path().into()).unwrap();
        assert!(service
            .save("http://unsafe/v1/media/generate".into())
            .is_err());
        let saved = service
            .save("https://media.example/v1/media/generate".into())
            .unwrap();
        assert_eq!(service.load().unwrap(), Some(saved));
        assert!(
            !std::fs::read_to_string(directory.path().join("media-gateway.json"))
                .unwrap()
                .contains("secret")
        );
    }
}
