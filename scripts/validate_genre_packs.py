"""Validate config-only genre breadth without changing core runtime code."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

import jsonschema

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "config"

SECTION_SCHEMAS = {
    "packs": "genre-template-v1.json",
    "constraint_profiles": "story-constraint-profile-v1.json",
    "agent_profiles": "story-agent-profile-v1.json",
    "retrieval_collections": "retrieval-collection-v1.json",
    "regression_manifests": "genre-regression-manifest-v1.json",
}
ID_FIELDS = {
    "packs": "template_id",
    "constraint_profiles": "profile_id",
    "agent_profiles": "profile_id",
    "retrieval_collections": "collection_id",
    "regression_manifests": "manifest_id",
}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def resolve_config(reference: str) -> Path:
    if (
        not reference
        or "\\" in reference
        or ":" in reference
        or any(part in {"", ".", ".."} for part in reference.split("/"))
    ):
        raise ValueError(f"unsafe config reference: {reference}")
    path = (CONFIG / reference).resolve()
    if CONFIG.resolve() not in path.parents or not path.is_file():
        raise ValueError(f"missing config reference: {reference}")
    return path


def all_cases() -> dict[tuple[str, str], dict[str, Any]]:
    cases = {}
    for path in (ROOT / "eval/cases").glob("*/cases.jsonl"):
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                case = json.loads(line)
                cases[(path.parent.name, case["case_id"])] = case
    return cases


def load_registry_documents(registry: dict[str, Any]) -> dict[str, dict[str, dict[str, Any]]]:
    documents = {}
    for section, schema_name in SECTION_SCHEMAS.items():
        schema = load_json(ROOT / "schemas" / schema_name)
        section_docs = {}
        for reference in registry[section]:
            document = load_json(resolve_config(reference))
            jsonschema.Draft202012Validator(schema).validate(document)
            identifier = document[ID_FIELDS[section]]
            if identifier in section_docs:
                raise ValueError(f"duplicate {section} id: {identifier}")
            section_docs[identifier] = document
        documents[section] = section_docs
    return documents


def validate_retrieval_sources(documents: dict[str, dict[str, dict[str, Any]]]) -> None:
    for collection in documents["retrieval_collections"].values():
        for source in collection["sources"]:
            path = (ROOT / source["path"]).resolve()
            if ROOT.resolve() not in path.parents or not path.is_file():
                raise ValueError(f"retrieval source missing: {source['source_id']}")
            if hashlib.sha256(path.read_bytes()).hexdigest() != source["content_sha256"]:
                raise ValueError(f"retrieval hash mismatch: {source['source_id']}")
            evidence = (ROOT / source["rights_evidence"]).resolve()
            if ROOT.resolve() not in evidence.parents or not evidence.is_file():
                raise ValueError(f"rights evidence missing: {source['source_id']}")


def validate_regression_cases(
    documents: dict[str, dict[str, dict[str, Any]]],
    cases: dict[tuple[str, str], dict[str, Any]],
) -> None:
    for manifest in documents["regression_manifests"].values():
        for case_ref in manifest["case_refs"]:
            case = cases.get((case_ref["split"], case_ref["case_id"]))
            if case is None or case["genre"] != manifest["genre"]:
                raise ValueError(
                    f"regression case mismatch: {manifest['manifest_id']}:{case_ref['case_id']}"
                )


def validate_pack(pack: dict[str, Any], documents: dict[str, dict[str, dict[str, Any]]]) -> None:
    profiles = [documents["constraint_profiles"].get(item) for item in pack["constraint_profiles"]]
    if any(profile is None for profile in profiles):
        raise ValueError(f"pack references unknown constraint profile: {pack['template_id']}")
    variants = {profile["episode_count_variant"] for profile in profiles if profile}
    if variants != {"short", "long"} or any(
        profile["content_form"] != pack["content_form"] for profile in profiles if profile
    ):
        raise ValueError(f"pack lacks shared-contract short/long variants: {pack['template_id']}")
    architect = documents["agent_profiles"].get(pack["agent_configuration"]["architect_profile"])
    reviewers = [documents["agent_profiles"].get(item) for item in pack["agent_configuration"]["reviewer_profiles"]]
    if architect is None or architect["role"] != "architect" or architect["genre"] != pack["genre"] or any(
        reviewer is None or reviewer["role"] != "reviewer" or reviewer["genre"] != pack["genre"]
        for reviewer in reviewers
    ):
        raise ValueError(f"pack agent profiles do not match genre: {pack['template_id']}")
    for profile in [architect, *reviewers]:
        if not (ROOT / profile["output_schema"]).is_file():
            raise ValueError(f"agent output schema missing: {profile['profile_id']}")
    if any(item not in documents["retrieval_collections"] for item in pack["retrieval_collections"]):
        raise ValueError(f"pack retrieval collection missing: {pack['template_id']}")
    regression = documents["regression_manifests"].get(pack["regression_manifest"])
    if regression is None or regression["genre"] != pack["genre"]:
        raise ValueError(f"pack regression manifest mismatch: {pack['template_id']}")
    if pack.get("status") == "promoted" and not pack.get("promoted_by_eval_run"):
        raise ValueError(f"promoted pack lacks hidden-gate evidence: {pack['template_id']}")
    if pack.get("status") != "promoted" and pack.get("promoted_by_eval_run") is not None:
        raise ValueError(f"unpromoted pack claims promotion evidence: {pack['template_id']}")


def validate_registry() -> dict[str, int]:
    registry = load_json(CONFIG / "genre-packs/registry-v1.json")
    registry_schema = load_json(ROOT / "schemas/genre-pack-registry-v1.json")
    jsonschema.Draft202012Validator(registry_schema).validate(registry)
    human_writing = load_json(resolve_config(registry["human_writing_profile"]))
    human_writing_schema = load_json(
        ROOT / "schemas/human-writing-profile-v1.json"
    )
    jsonschema.Draft202012Validator(human_writing_schema).validate(
        human_writing
    )
    documents = load_registry_documents(registry)
    validate_retrieval_sources(documents)
    validate_regression_cases(documents, all_cases())
    for pack in documents["packs"].values():
        validate_pack(pack, documents)
    counts = {section: len(values) for section, values in documents.items()}
    counts["human_writing_profiles"] = 1
    return counts


def main() -> int:
    counts = validate_registry()
    print(
        "Genre pack registry valid: "
        + ", ".join(f"{name}={count}" for name, count in counts.items())
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
