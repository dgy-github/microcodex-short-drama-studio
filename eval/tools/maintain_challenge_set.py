"""Plan quarterly challenge refresh and adversarial retirement without mutation."""

from __future__ import annotations

import json
from datetime import date, timedelta
from pathlib import Path
from typing import Any

import jsonschema

ROOT = Path(__file__).resolve().parents[2]


def maintenance_plan(
    state: dict[str, Any], policy: dict[str, Any], today: date
) -> dict[str, Any]:
    state_schema = json.loads(
        (ROOT / "schemas/adversarial-maintenance-state-v1.json").read_text("utf-8")
    )
    policy_schema = json.loads(
        (ROOT / "schemas/adversarial-lifecycle-policy-v1.json").read_text("utf-8")
    )
    jsonschema.Draft202012Validator(state_schema).validate(state)
    jsonschema.Draft202012Validator(policy_schema).validate(policy)
    last_refresh = date.fromisoformat(state["last_refresh"])
    refresh_due = today >= last_refresh + timedelta(
        days=policy["refresh"]["cadence_days"]
    )
    eligible = [
        item["case_id"]
        for item in state["production_failure_candidates"]
        if item["rights_verified"]
    ]
    refresh_ready = (
        refresh_due and len(eligible) >= policy["refresh"]["minimum_new_cases"]
    )
    immediate = set(policy["retirement"]["immediate_reasons"])
    windows = policy["retirement"]["saturation_windows"]
    retire, blocked = [], []
    for pair in state["pairs"]:
        if pair["status"] != "accepted_hard":
            continue
        reason = next((flag for flag in pair["flags"] if flag in immediate), None)
        recent = pair["measurement_windows"][-windows:]
        saturated = len(recent) == windows and all(
            item["detection_rate"] >= 0.98
            and item["localisation_rate"] >= 0.95
            for item in recent
        )
        reason = reason or ("metric_saturation" if saturated else None)
        if reason:
            action = {
                "pair_id": pair["pair_id"],
                "reason": reason,
                "replacement_pair_id": pair["replacement_pair_id"],
            }
            if (
                policy["retirement"]["replacement_required"]
                and not pair["replacement_pair_id"]
            ):
                blocked.append(action)
            else:
                retire.append(action)
    return {
        "schema": "adversarial-maintenance-plan/v1",
        "as_of": today.isoformat(),
        "refresh_due": refresh_due,
        "refresh_ready": refresh_ready,
        "eligible_production_failure_cases": eligible,
        "retire": retire,
        "retirement_blocked": blocked,
    }


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument(
        "--policy",
        type=Path,
        default=ROOT / "eval/adversarial/lifecycle-policy-v1.json",
    )
    parser.add_argument("--as-of", type=date.fromisoformat, default=date.today())
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError("maintenance plan output already exists")
    plan = maintenance_plan(
        json.loads(args.state.read_text("utf-8")),
        json.loads(args.policy.read_text("utf-8")),
        args.as_of,
    )
    args.output.write_text(
        json.dumps(plan, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
