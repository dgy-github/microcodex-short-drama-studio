use crate::{
    identity, validate_request, MediaExecutionError, MediaExecutor, MediaProvider, MediaRequest,
};
use serde_json::json;
use std::collections::HashSet;
use story_storage::media_events::MediaEventStore;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaRunOutcome {
    Completed(Box<crate::MediaGenerationResult>),
    Cancelled,
}

pub struct MediaRunService<P> {
    executor: MediaExecutor<P>,
    events: MediaEventStore,
}

impl<P: MediaProvider> MediaRunService<P> {
    pub fn new(executor: MediaExecutor<P>, events: MediaEventStore) -> Self {
        Self { executor, events }
    }

    pub async fn start(
        &self,
        run_id: &str,
        request: MediaRequest,
        cancel: watch::Receiver<bool>,
    ) -> Result<MediaRunOutcome, MediaExecutionError> {
        validate_request(&request)?;
        let (project_id, request_id, _) = identity(&request);
        let (_, inserted) = self.events.append_acceptance(
            project_id,
            run_id,
            request_id,
            json!({"request": request}),
        )?;
        if !inserted {
            return Err(MediaExecutionError::InvalidRequest);
        }
        self.execute(run_id, request, cancel).await
    }

    pub async fn resume(
        &self,
        run_id: &str,
        request: MediaRequest,
        cancel: watch::Receiver<bool>,
    ) -> Result<MediaRunOutcome, MediaExecutionError> {
        validate_request(&request)?;
        let (project_id, request_id, _) = identity(&request);
        if !self
            .recoverable()?
            .iter()
            .any(|(candidate_run, candidate)| {
                candidate_run == run_id && identity(candidate).1 == request_id
            })
        {
            return Err(MediaExecutionError::InvalidRequest);
        }
        self.events.append(
            project_id,
            run_id,
            request_id,
            "run.recovered",
            json!({"reason": "process_restart"}),
        )?;
        self.execute(run_id, request, cancel).await
    }

    pub fn recoverable(&self) -> Result<Vec<(String, MediaRequest)>, MediaExecutionError> {
        let events = self.events.replay(0)?;
        let terminal: HashSet<&str> = events
            .iter()
            .filter(|event| {
                matches!(
                    event.event_type.as_str(),
                    "run.completed" | "run.failed" | "run.cancelled"
                )
            })
            .map(|event| event.run_id.as_str())
            .collect();
        let mut recovered = Vec::new();
        for event in events
            .iter()
            .filter(|event| event.event_type == "run.accepted")
        {
            if terminal.contains(event.run_id.as_str())
                || recovered.iter().any(|(run_id, _)| run_id == &event.run_id)
            {
                continue;
            }
            let request = serde_json::from_value(event.payload["request"].clone())
                .map_err(|_| MediaExecutionError::InvalidRequest)?;
            validate_request(&request)?;
            recovered.push((event.run_id.clone(), request));
        }
        Ok(recovered)
    }

    async fn execute(
        &self,
        run_id: &str,
        request: MediaRequest,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<MediaRunOutcome, MediaExecutionError> {
        let (project_id, request_id, _) = identity(&request);
        self.events
            .append(project_id, run_id, request_id, "run.started", json!({}))?;
        if *cancel.borrow() {
            self.cancel(project_id, run_id, request_id)?;
            return Ok(MediaRunOutcome::Cancelled);
        }
        let execution = self.executor.execute(request.clone());
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => self.finish(project_id, run_id, request_id, result),
            changed = cancel.changed() => {
                if changed.is_ok() && *cancel.borrow() {
                    self.cancel(project_id, run_id, request_id)?;
                    Ok(MediaRunOutcome::Cancelled)
                } else {
                    self.finish(project_id, run_id, request_id, execution.await)
                }
            }
        }
    }

    fn finish(
        &self,
        project_id: &str,
        run_id: &str,
        request_id: &str,
        result: Result<crate::MediaGenerationResult, MediaExecutionError>,
    ) -> Result<MediaRunOutcome, MediaExecutionError> {
        match result {
            Ok(result) => {
                self.events.append_terminal(
                    project_id,
                    run_id,
                    request_id,
                    "run.completed",
                    serde_json::to_value(&result)
                        .map_err(|_| MediaExecutionError::InvalidOutput)?,
                )?;
                Ok(MediaRunOutcome::Completed(Box::new(result)))
            }
            Err(error) => {
                self.events.append_terminal(
                    project_id,
                    run_id,
                    request_id,
                    "run.failed",
                    json!({"error": error.to_string()}),
                )?;
                Err(error)
            }
        }
    }

    fn cancel(
        &self,
        project_id: &str,
        run_id: &str,
        request_id: &str,
    ) -> Result<(), MediaExecutionError> {
        self.events.append_terminal(
            project_id,
            run_id,
            request_id,
            "run.cancelled",
            json!({"reason": "user_requested"}),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GeneratedMedia, ImageGenerationRequest, MediaProviderError, ProviderFuture};
    use story_storage::media::MediaArtifactStore;

    struct FixtureProvider {
        delay: std::time::Duration,
    }

    impl crate::MediaProvider for FixtureProvider {
        fn generate<'a>(&'a self, _request: &'a MediaRequest) -> ProviderFuture<'a> {
            Box::pin(async move {
                tokio::time::sleep(self.delay).await;
                Ok(GeneratedMedia {
                    mime_type: "image/png".into(),
                    bytes: b"run-fixture-image".to_vec(),
                    provider: "fixture-provider".into(),
                    model: "fixture-model".into(),
                    cost_cny_fen: 1,
                    pricing_catalog_id: "fixture-pricing".into(),
                })
            })
        }
    }

    struct FailingProvider;

    impl crate::MediaProvider for FailingProvider {
        fn generate<'a>(&'a self, _request: &'a MediaRequest) -> ProviderFuture<'a> {
            Box::pin(async { Err(MediaProviderError::Failed) })
        }
    }

    fn request() -> MediaRequest {
        MediaRequest::Image(ImageGenerationRequest {
            schema: "image-generation-request/v1".into(),
            request_id: format!("img_{}", "a".repeat(32)),
            project_id: "project_1".into(),
            prompt_revision_id: "prompt_1".into(),
            prompt: "一张有动作的竖屏画面".into(),
            source_spans: vec!["story-package/scene-1".into()],
        })
    }

    fn service<P: crate::MediaProvider>(
        provider: P,
    ) -> (MediaRunService<P>, MediaEventStore, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let artifacts = MediaArtifactStore::open(directory.path().join("artifacts")).unwrap();
        let events = MediaEventStore::open(directory.path().join("events.jsonl")).unwrap();
        (
            MediaRunService::new(MediaExecutor::new(provider, artifacts), events.clone()),
            events,
            directory,
        )
    }

    #[tokio::test]
    async fn completed_run_retains_one_terminal_result() {
        let (service, events, _directory) = service(FixtureProvider {
            delay: std::time::Duration::ZERO,
        });
        let (_cancel, receiver) = watch::channel(false);
        let outcome = service
            .start("media_run_1", request(), receiver)
            .await
            .unwrap();
        assert!(matches!(outcome, MediaRunOutcome::Completed(_)));
        let events = events.replay(0).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.event_type.as_str(),
                    "run.completed" | "run.failed" | "run.cancelled"
                ))
                .count(),
            1
        );
        assert_eq!(events.last().unwrap().event_type, "run.completed");
        assert!(service.recoverable().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancellation_drops_provider_future_and_retains_no_media() {
        let (service, events, directory) = service(FixtureProvider {
            delay: std::time::Duration::from_secs(30),
        });
        let (cancel, receiver) = watch::channel(false);
        let run = service.start("media_run_2", request(), receiver);
        tokio::pin!(run);
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                cancel.send(true).unwrap();
            }
            _ = &mut run => panic!("slow provider completed before cancellation"),
        }
        assert_eq!(run.await.unwrap(), MediaRunOutcome::Cancelled);
        assert_eq!(
            events.replay(0).unwrap().last().unwrap().event_type,
            "run.cancelled"
        );
        assert!(!directory
            .path()
            .join("artifacts/media-projects/project_1")
            .exists());
    }

    #[tokio::test]
    async fn recovery_returns_only_accepted_non_terminal_runs() {
        let (service, events, _directory) = service(FailingProvider);
        let value = serde_json::to_value(request()).unwrap();
        events
            .append(
                "project_1",
                "media_run_pending",
                &format!("img_{}", "a".repeat(32)),
                "run.accepted",
                json!({"request": value}),
            )
            .unwrap();
        let recoverable = service.recoverable().unwrap();
        assert_eq!(recoverable.len(), 1);
        assert_eq!(recoverable[0].0, "media_run_pending");
        events
            .append_terminal(
                "project_1",
                "media_run_pending",
                &format!("img_{}", "a".repeat(32)),
                "run.cancelled",
                json!({}),
            )
            .unwrap();
        assert!(service.recoverable().unwrap().is_empty());
        let (_cancel, receiver) = watch::channel(false);
        assert!(matches!(
            service
                .resume("media_run_pending", request(), receiver)
                .await,
            Err(MediaExecutionError::InvalidRequest)
        ));
    }

    #[tokio::test]
    async fn duplicate_start_is_rejected_before_provider_execution() {
        let (service, _events, _directory) = service(FixtureProvider {
            delay: std::time::Duration::ZERO,
        });
        let (_cancel, first_receiver) = watch::channel(false);
        service
            .start("media_run_duplicate", request(), first_receiver)
            .await
            .unwrap();
        let (_cancel, second_receiver) = watch::channel(false);
        assert!(matches!(
            service
                .start("media_run_duplicate", request(), second_receiver)
                .await,
            Err(MediaExecutionError::InvalidRequest)
        ));
    }
}
