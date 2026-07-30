use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use story_provider::{OpenAiCompatibleProvider, ProviderRoute};
use uuid::Uuid;

use crate::credentials::CredentialService;
use crate::provider_settings::{ProviderRouteSettings, ProviderSettingsService};
use crate::CommandError;

const MIN_ITERATIONS: u8 = 3;
const MAX_ITERATIONS: u8 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderSoakResult {
    pub schema: &'static str,
    pub soak_id: String,
    pub iterations_per_provider: u8,
    pub status: &'static str,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub providers: Vec<ProviderSoakSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderSoakSummary {
    pub provider: String,
    pub model: String,
    pub route_fingerprint: String,
    pub status: &'static str,
    pub successful_requests: u8,
    pub failed_requests: u8,
    pub min_latency_ms: u64,
    pub average_latency_ms: u64,
    pub max_latency_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct Observation {
    latency_ms: u64,
    success: bool,
}

pub struct ProviderSoakService {
    root: PathBuf,
    active: AtomicBool,
}

impl ProviderSoakService {
    pub fn new(root: PathBuf) -> Result<Self, CommandError> {
        fs::create_dir_all(&root).map_err(|_| CommandError::provider_soak_unavailable())?;
        Ok(Self {
            root,
            active: AtomicBool::new(false),
        })
    }

    pub async fn run(
        &self,
        credentials: &CredentialService,
        settings: &ProviderSettingsService,
        iterations: u8,
    ) -> Result<ProviderSoakResult, CommandError> {
        validate_iterations(iterations)?;
        let _guard = self.acquire()?;

        let deepseek_settings = settings.load("deepseek", "default")?;
        let bailian_settings = settings.load("aliyun_bailian", "default")?;
        let deepseek_route = settings.route(credentials, "deepseek")?;
        let bailian_route = settings.route(credentials, "aliyun_bailian")?;
        let started_at_unix_ms = now_unix_ms()?;
        let client = OpenAiCompatibleProvider::new(Duration::from_secs(30))
            .map_err(|_| CommandError::provider_soak_failed())?;

        let deepseek = run_route(&client, &deepseek_route, &deepseek_settings, iterations).await;
        let bailian = run_route(&client, &bailian_route, &bailian_settings, iterations).await;
        let status = if deepseek.failed_requests == 0 && bailian.failed_requests == 0 {
            "ready"
        } else {
            "degraded"
        };
        let result = ProviderSoakResult {
            schema: "provider-soak-result/v1",
            soak_id: format!("soak_{}", Uuid::new_v4().simple()),
            iterations_per_provider: iterations,
            status,
            started_at_unix_ms,
            finished_at_unix_ms: now_unix_ms()?,
            providers: vec![deepseek, bailian],
        };
        self.persist(&result)?;
        Ok(result)
    }

    fn acquire(&self) -> Result<ActiveGuard<'_>, CommandError> {
        self.active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| CommandError::provider_soak_active())?;
        Ok(ActiveGuard(&self.active))
    }

    fn persist(&self, result: &ProviderSoakResult) -> Result<(), CommandError> {
        let target = self.root.join(format!("{}.json", result.soak_id));
        let partial = self.root.join(format!("{}.json.partial", result.soak_id));
        if target.exists() || partial.exists() {
            return Err(CommandError::provider_soak_unavailable());
        }
        let bytes = serde_json::to_vec_pretty(result)
            .map_err(|_| CommandError::provider_soak_unavailable())?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&partial)
            .map_err(|_| CommandError::provider_soak_unavailable())?;
        if file
            .write_all(&bytes)
            .and_then(|_| file.sync_all())
            .is_err()
        {
            drop(file);
            let _ = fs::remove_file(&partial);
            return Err(CommandError::provider_soak_unavailable());
        }
        drop(file);
        if fs::rename(&partial, &target).is_err() {
            let _ = fs::remove_file(&partial);
            return Err(CommandError::provider_soak_unavailable());
        }
        Ok(())
    }
}

struct ActiveGuard<'a>(&'a AtomicBool);

impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

async fn run_route(
    client: &OpenAiCompatibleProvider,
    route: &ProviderRoute,
    settings: &ProviderRouteSettings,
    iterations: u8,
) -> ProviderSoakSummary {
    let mut observations = Vec::with_capacity(iterations.into());
    for _ in 0..iterations {
        let started = Instant::now();
        let success = client
            .generate_json(
                route,
                "Return one compact JSON object only.",
                r#"Return exactly {"health":"ok"}."#,
            )
            .await
            .map(|output| output.artifact["health"] == "ok")
            .unwrap_or(false);
        observations.push(Observation {
            latency_ms: duration_ms(started.elapsed()),
            success,
        });
    }
    summarize(settings, &observations)
}

fn summarize(
    settings: &ProviderRouteSettings,
    observations: &[Observation],
) -> ProviderSoakSummary {
    let successful_requests = observations.iter().filter(|item| item.success).count() as u8;
    let failed_requests = observations.len() as u8 - successful_requests;
    let min_latency_ms = observations
        .iter()
        .map(|item| item.latency_ms)
        .min()
        .unwrap_or_default();
    let max_latency_ms = observations
        .iter()
        .map(|item| item.latency_ms)
        .max()
        .unwrap_or_default();
    let total_latency_ms = observations
        .iter()
        .fold(0_u64, |total, item| total.saturating_add(item.latency_ms));
    let average_latency_ms = total_latency_ms
        .checked_div(observations.len() as u64)
        .unwrap_or_default();
    ProviderSoakSummary {
        provider: settings.provider.clone(),
        model: settings.model.clone(),
        route_fingerprint: route_fingerprint(settings),
        status: if failed_requests == 0 {
            "ready"
        } else {
            "degraded"
        },
        successful_requests,
        failed_requests,
        min_latency_ms,
        average_latency_ms,
        max_latency_ms,
    }
}

fn route_fingerprint(settings: &ProviderRouteSettings) -> String {
    let material = format!(
        "{}\0{}\0{}\0{}",
        settings.provider, settings.profile, settings.endpoint, settings.model
    );
    format!("{:x}", Sha256::digest(material.as_bytes()))
}

fn validate_iterations(iterations: u8) -> Result<(), CommandError> {
    if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations) {
        return Err(CommandError::invalid_provider_soak());
    }
    Ok(())
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn now_unix_ms() -> Result<u64, CommandError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CommandError::provider_soak_unavailable())?
        .as_millis();
    u64::try_from(millis).map_err(|_| CommandError::provider_soak_unavailable())
}

pub fn default_provider_soak_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("MicrocodeX")
        .join("ShortDramaStudio")
        .join("provider-soaks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn settings() -> ProviderRouteSettings {
        ProviderRouteSettings {
            schema: "desktop-provider-route/v1".into(),
            provider: "deepseek".into(),
            profile: "default".into(),
            endpoint: "https://private.example/v1/chat/completions".into(),
            model: "configured-model".into(),
            thinking_disabled: false,
            source: "user".into(),
            record_id: Some("route_0123456789abcdef0123456789abcdef".into()),
            updated_at_unix_ms: Some(1),
        }
    }

    fn result() -> ProviderSoakResult {
        ProviderSoakResult {
            schema: "provider-soak-result/v1",
            soak_id: "soak_0123456789abcdef0123456789abcdef".into(),
            iterations_per_provider: 3,
            status: "degraded",
            started_at_unix_ms: 1,
            finished_at_unix_ms: 2,
            providers: vec![
                summarize(
                    &settings(),
                    &[
                        Observation {
                            latency_ms: 10,
                            success: true,
                        },
                        Observation {
                            latency_ms: 20,
                            success: false,
                        },
                        Observation {
                            latency_ms: 30,
                            success: true,
                        },
                    ],
                ),
                ProviderSoakSummary {
                    provider: "aliyun_bailian".into(),
                    model: "review-model".into(),
                    route_fingerprint: "a".repeat(64),
                    status: "ready",
                    successful_requests: 3,
                    failed_requests: 0,
                    min_latency_ms: 11,
                    average_latency_ms: 21,
                    max_latency_ms: 31,
                },
            ],
        }
    }

    #[test]
    fn bounded_observations_aggregate_without_sensitive_material() {
        assert!(validate_iterations(2).is_err());
        assert!(validate_iterations(3).is_ok());
        assert!(validate_iterations(20).is_ok());
        assert!(validate_iterations(21).is_err());
        let summary = &result().providers[0];
        assert_eq!(summary.successful_requests, 2);
        assert_eq!(summary.failed_requests, 1);
        assert_eq!(summary.average_latency_ms, 20);
        let encoded = serde_json::to_string(summary).unwrap();
        assert!(!encoded.contains("private.example"));
        assert!(!encoded.contains("chat/completions"));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("Return exactly"));
    }

    #[test]
    fn evidence_is_schema_valid_immutable_and_partial_free() {
        let directory = tempfile::tempdir().unwrap();
        let service = ProviderSoakService::new(directory.path().into()).unwrap();
        let result = result();
        service.persist(&result).unwrap();
        assert!(service.persist(&result).is_err());
        assert_eq!(
            fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(
                    |entry| entry.path().extension().and_then(|value| value.to_str())
                        == Some("partial")
                )
                .count(),
            0
        );
        let schema: serde_json::Value = serde_json::from_slice(
            &fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../schemas/provider-soak-result-v1.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&serde_json::to_value(result).unwrap()));
    }

    #[test]
    fn concurrent_soak_is_rejected_until_guard_is_released() {
        let directory = tempfile::tempdir().unwrap();
        let service = ProviderSoakService::new(directory.path().into()).unwrap();
        let guard = service.acquire().unwrap();
        assert!(service.acquire().is_err());
        drop(guard);
        assert!(service.acquire().is_ok());
    }
}
