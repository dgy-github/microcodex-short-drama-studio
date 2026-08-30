//! Trusted provider boundary. Concrete network clients arrive after protocol M0.

mod capability_host;
mod media_capability;
mod media_gateway;
mod openai_compatible;
mod package_validation;
mod pricing;

pub use capability_host::{
    CapabilityHost, CapabilityHostConfig, CapabilityHostError, CapabilityToken,
};
pub use media_gateway::{
    MediaGatewayClient, MediaGatewayError, MediaGatewayOutput, MediaGatewayRoute,
};
pub use openai_compatible::{
    OpenAiCompatibleProvider, ProviderOutput, ProviderRoute, ProviderRouteError,
};
pub use pricing::{PricingCatalog, PricingError, PricingQuote};

use std::fmt;
use zeroize::Zeroize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

pub trait StructuredTextProvider {
    type Error;

    fn generate(&self, schema: &str, prompt: &str) -> Result<(String, Usage), Self::Error>;
}

#[cfg(windows)]
const CREDENTIAL_SERVICE: &str = "com.microcodex.short-drama-studio.provider";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCredentialId {
    provider: String,
    profile: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialIdError {
    #[error("provider and profile must be 1-64 safe identifier characters")]
    Invalid,
}

impl ProviderCredentialId {
    pub fn new(
        provider: impl Into<String>,
        profile: impl Into<String>,
    ) -> Result<Self, CredentialIdError> {
        let provider = provider.into();
        let profile = profile.into();
        if !valid_component(&provider) || !valid_component(&profile) {
            return Err(CredentialIdError::Invalid);
        }
        Ok(Self { provider, profile })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn profile(&self) -> &str {
        &self.profile
    }

    #[cfg(windows)]
    fn account_name(&self) -> String {
        format!("{}/{}", self.provider, self.profile)
    }
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

pub struct ProviderSecret(Vec<u8>);

impl ProviderSecret {
    pub fn new(value: impl Into<Vec<u8>>) -> Result<Self, CredentialStoreError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CredentialStoreError::EmptySecret);
        }
        Ok(Self(value))
    }

    pub fn expose_secret(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl Drop for ProviderSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("credential secret must not be empty")]
    EmptySecret,
    #[error("credential store is unavailable")]
    Unavailable,
    #[error("credential entry is ambiguous")]
    Ambiguous,
    #[error("credential entry is invalid")]
    InvalidEntry,
}

pub trait ProviderCredentialStore {
    fn set(&self, id: &ProviderCredentialId, secret: &[u8]) -> Result<(), CredentialStoreError>;

    fn get(
        &self,
        id: &ProviderCredentialId,
    ) -> Result<Option<ProviderSecret>, CredentialStoreError>;

    fn delete(&self, id: &ProviderCredentialId) -> Result<bool, CredentialStoreError>;
}

#[cfg(windows)]
#[derive(Default)]
pub struct WindowsCredentialStore {
    gate: std::sync::Mutex<()>,
}

#[cfg(windows)]
impl WindowsCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn entry(id: &ProviderCredentialId) -> Result<keyring::Entry, CredentialStoreError> {
        keyring::Entry::new(CREDENTIAL_SERVICE, &id.account_name()).map_err(map_keyring_error)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, CredentialStoreError> {
        self.gate
            .lock()
            .map_err(|_| CredentialStoreError::Unavailable)
    }
}

#[cfg(windows)]
impl ProviderCredentialStore for WindowsCredentialStore {
    fn set(&self, id: &ProviderCredentialId, secret: &[u8]) -> Result<(), CredentialStoreError> {
        if secret.is_empty() {
            return Err(CredentialStoreError::EmptySecret);
        }
        let _guard = self.lock()?;
        Self::entry(id)?
            .set_secret(secret)
            .map_err(map_keyring_error)
    }

    fn get(
        &self,
        id: &ProviderCredentialId,
    ) -> Result<Option<ProviderSecret>, CredentialStoreError> {
        let _guard = self.lock()?;
        match Self::entry(id)?.get_secret() {
            Ok(secret) => Ok(Some(ProviderSecret(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_keyring_error(error)),
        }
    }

    fn delete(&self, id: &ProviderCredentialId) -> Result<bool, CredentialStoreError> {
        let _guard = self.lock()?;
        match Self::entry(id)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(error) => Err(map_keyring_error(error)),
        }
    }
}

#[cfg(windows)]
fn map_keyring_error(error: keyring::Error) -> CredentialStoreError {
    match error {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => {
            CredentialStoreError::Unavailable
        }
        keyring::Error::Ambiguous(_) => CredentialStoreError::Ambiguous,
        keyring::Error::NoEntry
        | keyring::Error::BadEncoding(_)
        | keyring::Error::TooLong(_, _)
        | keyring::Error::Invalid(_, _) => CredentialStoreError::InvalidEntry,
        _ => CredentialStoreError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_identity_accepts_provider_names_and_rejects_unsafe_values() {
        let id = ProviderCredentialId::new("aliyun_bailian", "default").unwrap();
        assert_eq!(id.provider(), "aliyun_bailian");
        assert_eq!(id.profile(), "default");
        assert_eq!(
            ProviderCredentialId::new("", "default"),
            Err(CredentialIdError::Invalid)
        );
        assert_eq!(
            ProviderCredentialId::new("qwen/provider", "default"),
            Err(CredentialIdError::Invalid)
        );
    }

    #[test]
    fn provider_secret_debug_is_redacted_and_exposure_is_explicit() {
        let secret = ProviderSecret(b"test-secret".to_vec());
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(secret.expose_secret(), b"test-secret");
    }
}
