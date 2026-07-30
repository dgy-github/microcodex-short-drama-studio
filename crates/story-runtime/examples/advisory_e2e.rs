#![cfg(windows)]

use jsonschema::Resource;
use serde_json::{json, Value};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;
use story_core::StoryJob;
use story_provider::{
    CapabilityHost, CapabilityHostConfig, CapabilityToken, ProviderRoute, ProviderSecret,
};
use story_runtime::{
    CommandAcceptance, IdempotencyKey, SidecarAuthToken, SidecarLaunchConfig, SidecarProcess,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repository = repository.canonicalize()?;
    let generation_endpoint = required_config("GENERATOR_ENDPOINT")?;
    let generation_model = required_config("GENERATOR_MODEL")?;
    let review_endpoint = required_config("REVIEW_ENDPOINT")?;
    let review_model = required_config("REVIEW_MODEL")?;
    let generation_secret = required_secret("GENERATOR_API_KEY")?;
    let review_secret = required_secret("JUDGE_API_KEY")?;
    let capability_token_value = "advisory-capability-token-20260728-000001";
    let capability_host = CapabilityHost::start(CapabilityHostConfig {
        generation: ProviderRoute::new(generation_endpoint, generation_model, generation_secret)?
            .with_thinking_disabled(),
        review: ProviderRoute::new(review_endpoint, review_model, review_secret)?
            .with_thinking_disabled(),
        package_schema_path: repository.join("schemas/story-package-v1.json"),
        token: CapabilityToken::new(capability_token_value)?,
        request_timeout: Duration::from_secs(240),
    })
    .await?;

    let workspace = repository
        .join("target/advisory-e2e")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&workspace)?;
    let python = repository.join(".venv/Scripts/python.exe");
    let sidecar = SidecarProcess::launch_with_capability(
        SidecarLaunchConfig::new(python, &workspace, Duration::from_secs(15))?,
        SidecarAuthToken::new("advisory-sidecar-token-20260728-00000001")?,
        &capability_host.endpoint(),
        &CapabilityToken::new(capability_token_value)?,
    )
    .await?;

    let job = story_job()?;
    let acceptance = sidecar
        .start_run(
            &job,
            &IdempotencyKey::new("advisory-e2e-run-20260728-000001")?,
        )
        .await?;
    let events = wait_for_terminal(&sidecar, &acceptance).await?;
    let terminal = events
        .iter()
        .rev()
        .find(|event| matches!(event.event_type.as_str(), "run.completed" | "run.failed"))
        .ok_or("terminal event missing")?;
    if terminal.event_type != "run.completed" {
        return Err(format!("workflow failed: {}", terminal.payload).into());
    }

    let result = sidecar.workflow_result(&acceptance).await?;
    validate_workflow_result(&repository, &result)?;
    let output_dir = repository
        .join("artifacts/advisory-runs")
        .join(&acceptance.run_id);
    std::fs::create_dir_all(&output_dir)?;
    write_json(&output_dir.join("workflow-result.json"), &result)?;
    write_json(&output_dir.join("story-package.json"), &result["package"])?;
    write_json(
        &output_dir.join("events.json"),
        &serde_json::to_value(&events)?,
    )?;

    println!(
        "{}",
        serde_json::to_string(&json!({
            "run_id": acceptance.run_id,
            "tasks_completed": 17,
            "reviews_completed": 5,
            "status": result["status"],
            "promotion": result["promotion"],
            "generation_model": result["provider_routes"]["generation"],
            "review_model": result["provider_routes"]["review"],
            "output_dir": output_dir
        }))?
    );
    sidecar.stop().await?;
    capability_host.stop().await?;
    Ok(())
}

fn required_secret(name: &str) -> Result<ProviderSecret, Box<dyn Error>> {
    let value = std::env::var_os(name).ok_or("required provider credential is missing")?;
    let bytes = value
        .to_str()
        .ok_or("provider credential is not valid UTF-8")?
        .as_bytes()
        .to_vec();
    Ok(ProviderSecret::new(bytes)?)
}

fn required_config(name: &str) -> Result<String, Box<dyn Error>> {
    let value = std::env::var(name)
        .map_err(|_| format!("required provider configuration {name} is missing"))?;
    if value.trim().is_empty() {
        return Err(format!("required provider configuration {name} is empty").into());
    }
    Ok(value)
}

fn story_job() -> Result<StoryJob, serde_json::Error> {
    serde_json::from_value(json!({
        "schema": "story-job/v1",
        "job_id": "job_advisory_e2e_20260728",
        "content_form": "scripted_short_drama",
        "input": "停电后的老旧商场里，一名维修工发现故障电梯中被困的是二十年前抛下他的父亲；商场开门前，他必须在救人、追问真相和保住工作之间作出选择。",
        "genre_mode": "fixed",
        "allowed_genres": ["family", "suspense"],
        "audience": "25-45",
        "format": {"episodes": 6, "minutes_per_episode": 2},
        "content_limits": ["不美化遗弃行为", "不使用超自然解释"],
        "budget": {
            "max_tokens": 90000,
            "max_cny_fen": 1200,
            "deadline_seconds": 900
        }
    }))
}

async fn wait_for_terminal(
    sidecar: &SidecarProcess,
    acceptance: &CommandAcceptance,
) -> Result<Vec<story_runtime::EventEnvelope>, Box<dyn Error>> {
    let mut last_completed = 0;
    for _ in 0..1800 {
        let events = sidecar.replay_events(acceptance, None).await?;
        let completed = events
            .iter()
            .filter(|event| event.event_type == "task.completed")
            .count();
        if completed > last_completed {
            println!("workflow progress: {completed}/17 tasks completed");
            last_completed = completed;
        }
        if events
            .iter()
            .any(|event| matches!(event.event_type.as_str(), "run.completed" | "run.failed"))
        {
            return Ok(events);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err("workflow terminal event timed out".into())
}

fn validate_workflow_result(repository: &Path, result: &Value) -> Result<(), Box<dyn Error>> {
    let workflow_schema = read_json(&repository.join("schemas/story-workflow-result-v1.json"))?;
    let package_schema = read_json(&repository.join("schemas/story-package-v1.json"))?;
    let review_schema = read_json(&repository.join("schemas/story-review-record-v1.json"))?;
    let validator = jsonschema::options()
        .with_resources(
            [
                (
                    "https://microcodex.local/schemas/story-package-v1.json",
                    Resource::from_contents(package_schema)?,
                ),
                (
                    "https://microcodex.local/schemas/story-review-record-v1.json",
                    Resource::from_contents(review_schema)?,
                ),
            ]
            .into_iter(),
        )
        .build(&workflow_schema)?;
    validator
        .validate(result)
        .map_err(|error| format!("workflow result schema validation failed: {error}"))?;
    let expected_ids = (1..=17)
        .map(|index| format!("t{index:02}"))
        .collect::<Vec<_>>();
    let actual_ids = result["tasks"]
        .as_array()
        .ok_or("tasks missing")?
        .iter()
        .filter_map(|task| task["task_id"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    if actual_ids != expected_ids {
        return Err("workflow task evidence is incomplete".into());
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn write_json(path: &Path, value: &Value) -> Result<(), Box<dyn Error>> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
