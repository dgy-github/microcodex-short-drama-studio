"""Validate release configuration without producing or signing installers."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.1.0-alpha.1"
INSTALLER_VERSION = "0.1.0-1"
RELEASE_ASSETS = (
    "rust-toolchain.toml", ".nvmrc", ".python-version",
    "scripts/build_windows_sidecar.ps1", "scripts/verify_pinned_python_dependency.py",
    "scripts/build_windows_release.ps1", "scripts/build_signed_windows_release.ps1",
    "scripts/verify_windows_bundle.ps1", "apps/desktop/src-tauri/windows/nsis-hooks.nsh",
    "apps/desktop/src-tauri/resources/story-sidecar/README.txt", "docs/OPERATOR_GUIDE.md",
    "docs/UPGRADE_ROLLBACK_POLICY.md", "docs/SECURITY_REVIEW.md", "docs/INCIDENT_RUNBOOK.md",
    "schemas/windows-release-evidence-v1.json", "schemas/distribution-license-policy-v1.json",
    "config/distribution-license-policy-v1.json", "third_party/licenses/README.md",
    "schemas/provider-health-v1.json",
)


def load_json(relative: str) -> dict[str, Any]:
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


def check_versions_and_bundle(errors: list[str]) -> None:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    desktop = tomllib.loads((ROOT / "apps/desktop/src-tauri/Cargo.toml").read_text(encoding="utf-8"))
    package = load_json("apps/desktop/package.json")
    tauri = load_json("apps/desktop/src-tauri/tauri.conf.json")
    versions = {cargo["workspace"]["package"]["version"], desktop["package"]["version"], package["version"]}
    if versions != {VERSION}:
        errors.append(f"application versions diverge: {sorted(versions)}")
    if tauri["version"] != INSTALLER_VERSION:
        errors.append(f"installer version must be MSI-compatible {INSTALLER_VERSION}")
    bundle = tauri.get("bundle", {})
    if not bundle.get("active") or set(bundle.get("targets", [])) != {"msi", "nsis"}:
        errors.append("MSI and NSIS bundle targets must both be active")
    if not {"resources/README.txt", "resources/story-sidecar"} <= set(bundle.get("resources", [])):
        errors.append("bundled sidecar resources are not declared")
    if bundle.get("windows", {}).get("nsis", {}).get("installerHooks") != "windows/nsis-hooks.nsh":
        errors.append("NSIS WebView2 loader hook is not declared")


def check_assets(errors: list[str]) -> None:
    for relative in RELEASE_ASSETS:
        if not (ROOT / relative).is_file():
            errors.append(f"missing release asset: {relative}")


def require_markers(errors: list[str], text: str, markers: tuple[str, ...], label: str) -> None:
    for required in markers:
        if required not in text:
            errors.append(f"{label} lacks: {required}")


def check_scripts(errors: list[str]) -> None:
    release = (ROOT / "scripts/build_windows_release.ps1").read_text(encoding="utf-8")
    require_markers(errors, release, (
        "signtool.exe", "verify /pa", "Find-SignTool", "https://timestamp.digicert.com",
        "Get-FileHash", "signed = $signed", "signing = $signing", "Release builds require a clean worktree",
        "source_diff_sha256", "ls-files --others --exclude-standard", "build_dependency_inventory.py",
        "AllowUnlicensedForLocalVerification", "Distribution license review is unresolved",
        "installer_release_eligible", "license_policy_sha256", "Rust toolchain must be 1.88.0",
        "Refusing to clean bundle output", "verify_windows_bundle.ps1", "bundle_smoke = $bundleSmoke",
        "Bundled sidecar is missing story-package-v1.json", "[IO.File]::WriteAllText",
        "Expected exactly one x64 WebView2Loader.dll before bundling", "target\\release\\WebView2Loader.dll",
    ), "release script")
    sidecar = (ROOT / "scripts/build_windows_sidecar.ps1").read_text(encoding="utf-8")
    require_markers(errors, sidecar, (
        'child.Name -eq "README.txt"', "expected directory", "--force-reinstall", "--verify-installed",
        "--add-data", "Packaged sidecar is missing story-package-v1.json",
    ), "sidecar build script")
    signed = (ROOT / "scripts/build_signed_windows_release.ps1").read_text(encoding="utf-8")
    require_markers(errors, signed, (
        "WINDOWS_SIGNING_PFX_BASE64", "WINDOWS_SIGNING_PFX_PASSWORD", "Import-PfxCertificate",
        "EphemeralKeySet", "already exists in CurrentUser\\My", "Cert:\\CurrentUser\\My", "finally",
        "Remove-Item -LiteralPath $certificatePath", "Remove-Item -LiteralPath $pfxPath",
    ), "signed release script")
    workflow = (ROOT / ".github/workflows/governance.yml").read_text(encoding="utf-8")
    require_markers(errors, workflow, (
        "windows-release-smoke:", "github.event_name == 'workflow_dispatch'",
        r".\scripts\build_windows_release.ps1", "AllowUnlicensedForLocalVerification",
        r".\scripts\build_signed_windows_release.ps1", "WINDOWS_SIGNING_PFX_BASE64",
        "WINDOWS_SIGNING_PFX_PASSWORD", "target/release-evidence/*.json",
    ), "release workflow")


def check_evidence_schema(errors: list[str]) -> None:
    schema = load_json("schemas/windows-release-evidence-v1.json")
    required = set(schema.get("required", []))
    for field, message in (
        ("bundle_smoke", "release evidence does not require bundle_smoke"),
        ("signing", "release evidence does not require signing provenance"),
        ("distribution_license", "release evidence does not require license provenance"),
        ("installer_release_eligible", "release evidence does not require installer eligibility"),
    ):
        if field not in required:
            errors.append(message)
    smoke = schema.get("properties", {}).get("bundle_smoke", {}).get("properties", {})
    for result in ("msi_admin_extract", "sidecar_protocol", "webview2_loader_present", "story_schema_present", "desktop_launch"):
        if smoke.get(result, {}).get("const") is not True:
            errors.append(f"bundle smoke evidence must require {result}=true")


def check_license_policy(errors: list[str]) -> None:
    policy = load_json("config/distribution-license-policy-v1.json")
    entries = {(item.get("ecosystem"), item.get("name")): item for item in policy.get("dependencies", [])}
    if policy.get("schema") != "distribution-license-policy/v1" or len(entries) != 2:
        errors.append("distribution license policy shape is invalid")
        return
    campaign = entries.get(("python", "campaign-muti-agent"), {})
    if (campaign.get("revision") != "1d935714449d18cad5bdc6711a498297ed73a5fb"
        or campaign.get("approved_for_distribution") is not True or campaign.get("license") != "MIT"
        or campaign.get("evidence_path") != "third_party/licenses/campaign-muti-agent/LICENSE"):
        errors.append("Campaign distribution license policy is inconsistent")
    css = entries.get(("npm", "css-value"), {})
    if (css.get("version") != "0.0.1"
        or css.get("integrity") != "sha512-FUV3xaJ63buRLgHrLQVlVgQnQdR4yqdLGaDu7g8CQcWjInDfM9plBTPI9FRfpahju1UBSaMckeb2/46ApS/V1Q=="
        or css.get("approved_for_distribution") is not True or css.get("license") != "MIT"
        or css.get("evidence_path") != "third_party/licenses/css-value-0.0.1/Readme.md"):
        errors.append("css-value distribution license policy is inconsistent")


def check() -> list[str]:
    errors: list[str] = []
    check_versions_and_bundle(errors)
    check_assets(errors)
    check_scripts(errors)
    check_evidence_schema(errors)
    check_license_policy(errors)
    return errors


def main() -> int:
    errors = check()
    if errors:
        print("\n".join(errors))
        return 1
    print("Release configuration check passed (artifact signing not executed)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
