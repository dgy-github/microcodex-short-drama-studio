import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import jsonschema
from build_dependency_inventory import (
    build_inventory,
    load_license_policy,
    pinned_dependencies,
    validate_license_policy,
)

ROOT = Path(__file__).resolve().parents[1]


class DependencyInventoryTests(unittest.TestCase):
    def test_inventory_is_lockfile_backed_and_deterministic(self) -> None:
        inventory = build_inventory()
        self.assertEqual(inventory["schema"], "dependency-inventory/v1")
        self.assertEqual(
            {package["ecosystem"] for package in inventory["packages"]},
            {"cargo", "npm", "python"},
        )
        self.assertTrue(inventory["distribution_cleared"])
        self.assertEqual(inventory["review_required"], [])
        self.assertEqual(
            len(inventory["license_policy_sha256"]),
            64,
        )
        campaign = next(
            package
            for package in inventory["packages"]
            if package["name"] == "campaign-muti-agent"
        )
        self.assertEqual(
            campaign["version"],
            "1d935714449d18cad5bdc6711a498297ed73a5fb",
        )
        self.assertEqual(campaign["license"], "MIT")
        self.assertEqual(campaign["license_source"], "reviewed-policy")
        css_value = next(
            package
            for package in inventory["packages"]
            if package["ecosystem"] == "npm" and package["name"] == "css-value"
        )
        self.assertEqual(css_value["version"], "0.0.1")
        self.assertEqual(css_value["license"], "MIT")
        self.assertEqual(css_value["license_source"], "reviewed-policy")
        self.assertEqual(inventory, build_inventory())

    def test_policy_rejects_pin_drift(self) -> None:
        policy, _ = load_license_policy()
        drifted = copy.deepcopy(policy)
        drifted["dependencies"][0]["revision"] = "a" * 40
        with self.assertRaisesRegex(ValueError, "pin drift"):
            validate_license_policy(drifted, pinned_dependencies())

    def test_policy_rejects_npm_pin_drift(self) -> None:
        policy, _ = load_license_policy()
        drifted = copy.deepcopy(policy)
        css_value = next(
            entry for entry in drifted["dependencies"] if entry["name"] == "css-value"
        )
        css_value["integrity"] = "sha512-" + "A" * 88
        with self.assertRaisesRegex(ValueError, "pin drift"):
            validate_license_policy(drifted, pinned_dependencies())

    def test_policy_rejects_mixed_ecosystem_pin_fields(self) -> None:
        policy, _ = load_license_policy()
        mixed = copy.deepcopy(policy)
        css_value = next(
            entry for entry in mixed["dependencies"] if entry["name"] == "css-value"
        )
        css_value["source"] = "https://registry.npmjs.org/css-value"
        css_value["revision"] = "a" * 40
        with self.assertRaisesRegex(ValueError, "fields are invalid"):
            validate_license_policy(mixed, pinned_dependencies())

    def test_schema_rejects_missing_or_mixed_pin_fields(self) -> None:
        schema = json.loads(
            (ROOT / "schemas/distribution-license-policy-v1.json").read_text(
                encoding="utf-8"
            )
        )
        policy, _ = load_license_policy()
        jsonschema.validate(policy, schema)

        missing = copy.deepcopy(policy)
        missing["dependencies"][1].pop("integrity")
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(missing, schema)

        mixed = copy.deepcopy(policy)
        mixed["dependencies"][1]["revision"] = "a" * 40
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(mixed, schema)

    def test_unapproved_policy_cannot_claim_a_license(self) -> None:
        policy, _ = load_license_policy()
        claimed = copy.deepcopy(policy)
        entry = claimed["dependencies"][0]
        entry.update(
            {
                "approved_for_distribution": False,
                "evidence_path": None,
                "evidence_sha256": None,
                "reviewed_by": None,
                "reviewed_at": None,
            }
        )
        with self.assertRaisesRegex(ValueError, "must remain empty"):
            validate_license_policy(claimed, pinned_dependencies())

    def test_approved_policy_requires_and_accepts_hashed_evidence(self) -> None:
        policy, _ = load_license_policy()
        approved = copy.deepcopy(policy)
        approved["dependencies"] = [approved["dependencies"][0]]
        entry = approved["dependencies"][0]
        entry.update(
            {
                "license": "MIT",
                "evidence_path": "third_party/licenses/campaign/LICENSE",
                "approved_for_distribution": True,
                "reviewed_by": "release-owner",
                "reviewed_at": "2026-07-28T12:00:00Z",
            }
        )
        pinned = pinned_dependencies()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            license_root = root / "third_party/licenses"
            evidence = license_root / "campaign/LICENSE"
            evidence.parent.mkdir(parents=True)
            evidence.write_text("authoritative license text\n", encoding="utf-8")
            entry["evidence_sha256"] = hashlib.sha256(
                evidence.read_bytes()
            ).hexdigest()
            with (
                patch("build_dependency_inventory.ROOT", root),
                patch("build_dependency_inventory.LICENSE_ROOT", license_root),
            ):
                entries = validate_license_policy(
                    approved,
                    pinned,
                )
        self.assertTrue(
            entries[("python", "campaign-muti-agent")][
                "approved_for_distribution"
            ]
        )


if __name__ == "__main__":
    unittest.main()
