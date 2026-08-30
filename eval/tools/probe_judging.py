"""Validation and normalization of stage-0 judge responses."""
from __future__ import annotations
import json
from typing import Any
from probe_transport import request, valid_line_spans

def validate_judgment(
    value: dict[str, Any],
    first: dict[str, Any],
    second: dict[str, Any],
    dimension_ids: list[str],
) -> None:
    allowed = {"A": valid_line_spans(first), "B": valid_line_spans(second)}
    for label in ("A", "B"):
        block = value.get(label)
        if not isinstance(block, dict):
            raise ValueError(f"{label} must be an object of dimensions")
        missing = [d for d in dimension_ids if d not in block]
        if missing:
            raise ValueError(f"{label} missing dimensions: {missing}")
        for dimension in dimension_ids:
            entry = block[dimension]
            score = entry.get("score")
            if not isinstance(score, int) or not 1 <= score <= 5:
                raise ValueError(f"{label}.{dimension}.score must be 1-5")
            if not entry.get("reason") or not entry.get("spans"):
                raise ValueError(f"{label}.{dimension} requires a reason and spans")
            invalid = set(entry["spans"]) - allowed[label]
            if invalid:
                raise ValueError(
                    f"{label}.{dimension} invalid spans: {sorted(invalid)}"
                )
    if value.get("preferred") not in {"A", "B", "tie"}:
        raise ValueError("preferred must be A, B, or tie")


def normalize_span_list(
    spans: list[Any], artifact: dict[str, Any], allowed: set[str]
) -> list[Any]:
    """Map each cited span back to an unambiguous owning addressable node."""
    collections = {
        "story-package/characters": artifact["characters"],
        "story-package/beats": artifact["beats"],
        "story-package/episodes": artifact["episodes"],
        "story-package/scenes": artifact["scenes"],
    }
    normalized: list[Any] = []
    for span in spans:
        # glm sometimes pluralises the fixed prefix ("story-packages/char-1");
        # repair that one typo before any other matching
        if isinstance(span, str) and span.startswith("story-packages/"):
            span = "story-package/" + span[len("story-packages/"):]
        if span in collections:
            normalized.extend(
                f"story-package/{node['node_id']}" for node in collections[span]
            )
            continue
        if span in {
            "story-package/production",
            "story-package/production/locations",
        }:
            normalized.extend(
                f"story-package/{scene['node_id']}"
                for scene in artifact["scenes"]
            )
            if span != "story-package/production":
                continue
        if span in {
            "story-package/production",
            "story-package/production/speaking_cast",
        }:
            normalized.extend(artifact["production"]["speaking_cast"])
            continue
        candidate = span.split(".", 1)[0] if isinstance(span, str) else span
        while (
            isinstance(candidate, str)
            and candidate not in allowed
            and "/" in candidate.removeprefix("story-package/")
        ):
            candidate = candidate.rsplit("/", 1)[0]
        normalized.append(candidate if candidate in allowed else span)
    return list(dict.fromkeys(normalized))


def normalize_owned_field_spans(
    value: dict[str, Any],
    first: dict[str, Any],
    second: dict[str, Any],
) -> None:
    """Map a cited field back to its unambiguous owning addressable node."""
    allowed = {"A": valid_line_spans(first), "B": valid_line_spans(second)}
    artifacts = {"A": first, "B": second}
    for label in ("A", "B"):
        for entry in value.get(label, {}).values():
            if not isinstance(entry, dict) or not isinstance(entry.get("spans"), list):
                continue
            entry["spans"] = normalize_span_list(
                entry["spans"], artifacts[label], allowed[label]
            )


def request_validated(
    route: dict[str, Any],
    model: str,
    system: str,
    api_key: str | None,
    first: dict[str, Any],
    second: dict[str, Any],
    temperature: float,
    dimension_ids: list[str],
    max_attempts: int = 3,
) -> dict[str, Any]:
    validation_error: str | None = None
    for attempt in range(1, max_attempts + 1):
        try:
            sample = request(
                route,
                model,
                system,
                api_key,
                first,
                second,
                temperature,
                validation_error,
            )
        except (json.JSONDecodeError, KeyError, TypeError, ValueError) as error:
            # ValueError covers the empty-content and fence-trim cases a relay
            # produces; they are as retryable as malformed JSON
            validation_error = f"{type(error).__name__}: {error}"
            if attempt == max_attempts:
                raise
            print(
                f"RETRY {model} via {route['provider']}: "
                f"unparseable judge output ({validation_error})"
            )
            continue
        normalize_owned_field_spans(sample, first, second)
        try:
            validate_judgment(sample, first, second, dimension_ids)
            return sample
        except ValueError as error:
            validation_error = str(error)
            if attempt == max_attempts:
                raise
            print(
                f"RETRY {model} via {route['provider']}: "
                f"invalid judge output ({validation_error})"
            )
    raise AssertionError("unreachable")
