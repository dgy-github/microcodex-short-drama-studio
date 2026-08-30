"""Pure validation for the typed StartRun wire command and genre context."""

from __future__ import annotations

from typing import Any

START_RUN_SCHEMA = "start-run-command/v1"
STORY_JOB_SCHEMA = "story-job/v1"
SUPPORTED_CONTENT_FORMS = frozenset({"scripted_short_drama"})


def validate_start_command(command: dict[str, Any]) -> None:
    if (
        not {"schema", "job"} <= set(command)
        or not set(command) <= {"schema", "job", "genre_context"}
        or command.get("schema") != START_RUN_SCHEMA
    ):
        raise ValueError("invalid StartRun command")
    job = command.get("job")
    if not isinstance(job, dict):
        raise ValueError("invalid StoryJob")
    if (
        job.get("schema") != STORY_JOB_SCHEMA
        or job.get("content_form") not in SUPPORTED_CONTENT_FORMS
    ):
        raise ValueError("invalid StoryJob")
    if not isinstance(job.get("job_id"), str) or not job["job_id"].strip():
        raise ValueError("invalid StoryJob")
    if "genre_context" in command:
        validate_genre_context(command["genre_context"])


def validate_genre_context(context: Any) -> None:
    required_fields = {
        "schema", "pack_id", "constraint_profile_id", "genre",
        "architect_directives", "reviewer_directives", "retrieval_sources",
    }
    allowed_fields = required_fields | {"human_writing"}
    if (
        not isinstance(context, dict)
        or not required_fields <= set(context)
        or not set(context) <= allowed_fields
        or context.get("schema") != "genre-context/v1"
    ):
        raise ValueError("invalid genre context")
    for field in ("pack_id", "constraint_profile_id", "genre"):
        if not isinstance(context.get(field), str) or not context[field].strip():
            raise ValueError("invalid genre context")
    for field in ("architect_directives", "reviewer_directives"):
        values = context.get(field)
        if (
            not isinstance(values, list)
            or not values
            or any(not isinstance(value, str) or not value.strip() for value in values)
        ):
            raise ValueError("invalid genre context")
    sources = context.get("retrieval_sources")
    if not isinstance(sources, list):
        raise ValueError("invalid genre context")
    for source in sources:
        if not isinstance(source, dict) or set(source) != {
            "source_id", "license_id", "content_sha256", "usage",
        }:
            raise ValueError("invalid genre context")
        if any(
            not isinstance(source.get(field), str) or not source[field].strip()
            for field in source
        ):
            raise ValueError("invalid genre context")
    if "human_writing" in context:
        validate_human_writing_context(context["human_writing"])


def validate_human_writing_context(context: Any) -> None:
    if not isinstance(context, dict) or set(context) != {
        "profile_id", "task_directives",
    }:
        raise ValueError("invalid human writing context")
    if not isinstance(context.get("profile_id"), str) or not context["profile_id"].strip():
        raise ValueError("invalid human writing context")
    task_directives = context.get("task_directives")
    expected_tasks = {"t07", "t10", "t12", "t15", "t16"}
    if not isinstance(task_directives, dict) or set(task_directives) != expected_tasks:
        raise ValueError("invalid human writing context")
    for directives in task_directives.values():
        if (
            not isinstance(directives, list)
            or not directives
            or any(not isinstance(item, str) or not item.strip() for item in directives)
        ):
            raise ValueError("invalid human writing context")
