#![cfg(windows)]

use std::path::PathBuf;
use std::time::Duration;
use story_core::StoryJob;
use story_runtime::{
    IdempotencyKey, SidecarAuthToken, SidecarLaunchConfig, SidecarProcess, SidecarState,
};

async fn remove_workspace(path: &std::path::Path) {
    for _ in 0..20 {
        match std::fs::remove_dir_all(path) {
            Ok(()) => return,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    std::fs::remove_dir_all(path).unwrap();
}

fn story_job() -> StoryJob {
    serde_json::from_value(serde_json::json!({
        "schema": "story-job/v1",
        "job_id": "job_sidecar_smoke",
        "content_form": "scripted_short_drama",
        "input": "两名维修工必须在商场开门前修好同一部故障电梯。",
        "genre_mode": "auto",
        "allowed_genres": ["family"],
        "audience": "25-45",
        "format": {"episodes": 8, "minutes_per_episode": 2},
        "content_limits": [],
        "budget": {
            "max_tokens": 100000,
            "max_cny_fen": 1000,
            "deadline_seconds": 600
        }
    }))
    .unwrap()
}

#[tokio::test]
#[ignore = "starts either the installed Python or bundled Campaign sidecar on localhost"]
async fn duplicate_start_run_and_last_event_id_replay() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace = repository
        .join("target/sidecar-smoke")
        .join(std::process::id().to_string());
    if workspace.exists() {
        remove_workspace(&workspace).await;
    }
    std::fs::create_dir_all(&workspace).unwrap();
    let config = match std::env::var_os("MICROCODEX_TEST_BUNDLED_SIDECAR") {
        Some(executable) => SidecarLaunchConfig::bundled(
            PathBuf::from(executable),
            &workspace,
            Duration::from_secs(20),
        ),
        None => SidecarLaunchConfig::new(
            repository.join(".venv/Scripts/python.exe"),
            &workspace,
            Duration::from_secs(10),
        ),
    }
    .unwrap();
    let token = SidecarAuthToken::new("integration-test-token-with-at-least-32-bytes").unwrap();

    let process = SidecarProcess::launch(config, token).await.unwrap();
    assert_eq!(process.state(), SidecarState::Ready);
    assert!(process.base_url().starts_with("http://127.0.0.1:"));
    assert!(process.process_id().is_some());
    process.health().await.unwrap();

    let job = story_job();
    let key = IdempotencyKey::new("start-run-smoke-key-00000001").unwrap();
    let first = process.start_run(&job, &key).await.unwrap();
    let duplicate = process.start_run(&job, &key).await.unwrap();
    assert_eq!(duplicate, first);

    let initial = process.replay_events(&first, None).await.unwrap();
    assert_eq!(initial.len(), 2);
    assert_eq!(initial[0].event_type, "run.accepted");
    assert_eq!(initial[1].event_type, "task.queued");
    assert_eq!(initial[1].task_id.as_deref(), Some("t01"));

    let resumed = process
        .replay_events(&first, Some(initial[0].seq))
        .await
        .unwrap();
    assert_eq!(resumed, vec![initial[1].clone()]);

    let cancelled = process.cancel_run(&first).await.unwrap();
    let duplicate_cancel = process.cancel_run(&first).await.unwrap();
    assert_eq!(cancelled, duplicate_cancel);
    assert_eq!(cancelled.event_type, "run.cancelled");
    let after_cancel = process
        .replay_events(&first, Some(initial[1].seq))
        .await
        .unwrap();
    assert_eq!(after_cancel, vec![cancelled]);

    process.stop().await.unwrap();
    remove_workspace(&workspace).await;
}
