import copy
import json
import unittest
from pathlib import Path

import jsonschema
from check_release_configuration import check

ROOT = Path(__file__).resolve().parents[1]


class ReleaseConfigurationTests(unittest.TestCase):
    def test_release_configuration_is_internally_consistent(self) -> None:
        self.assertEqual(check(), [])

    def test_release_evidence_requires_successful_bundle_smoke(self) -> None:
        schema = json.loads(
            (ROOT / "schemas/windows-release-evidence-v1.json").read_text(
                encoding="utf-8"
            )
        )
        evidence = {
            "schema": "windows-release-evidence/v1",
            "version": "0.1.0-alpha.1",
            "installer_version": "0.1.0-1",
            "commit": "a" * 40,
            "dirty": False,
            "source_diff_sha256": "b" * 64,
            "tools": {"rust": "rustc", "node": "node", "python": "python"},
            "lockfiles": ["Cargo.lock", "package-lock.json", "pyproject.toml"],
            "signed": False,
            "signing": None,
            "distribution_license": {
                "cleared": False,
                "review_required": ["campaign-muti-agent"],
                "policy": "config/distribution-license-policy-v1.json",
                "policy_sha256": "f" * 64,
            },
            "installer_release_eligible": False,
            "bundle_smoke": {
                "msi_path": "target/release/app.msi",
                "msi_admin_extract": True,
                "sidecar_protocol": True,
                "webview2_loader_present": True,
                "story_schema_present": True,
                "desktop_launch": True,
                "desktop_alive_seconds": 5,
                "sidecar_sha256": "c" * 64,
                "desktop_sha256": "d" * 64,
                "webview2_loader_sha256": "1" * 64,
                "story_schema_sha256": "2" * 64,
            },
            "artifacts": [
                {
                    "path": "target/release/app.msi",
                    "sha256": "e" * 64,
                    "bytes": 1,
                }
            ],
        }
        jsonschema.validate(evidence, schema)

        failed = copy.deepcopy(evidence)
        failed["bundle_smoke"]["sidecar_protocol"] = False
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(failed, schema)

        signed = copy.deepcopy(evidence)
        signed["signed"] = True
        signed["signing"] = {
            "certificate_thumbprint": "A" * 40,
            "timestamp_url": "https://timestamp.digicert.com",
            "verified": True,
        }
        signed["distribution_license"] = {
            "cleared": True,
            "review_required": [],
            "policy": "config/distribution-license-policy-v1.json",
            "policy_sha256": "f" * 64,
        }
        jsonschema.validate(signed, schema)

        inconsistent = copy.deepcopy(evidence)
        inconsistent["signed"] = True
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(inconsistent, schema)

        eligible = copy.deepcopy(signed)
        eligible["installer_release_eligible"] = True
        jsonschema.validate(eligible, schema)

        dirty_eligible = copy.deepcopy(eligible)
        dirty_eligible["dirty"] = True
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(dirty_eligible, schema)


if __name__ == "__main__":
    unittest.main()
