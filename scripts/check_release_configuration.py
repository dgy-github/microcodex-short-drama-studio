"""Validate release configuration without producing or signing installers."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.1.0-alpha.1"
INSTALLER_VERSION = "0.1.0-1"


def check() -> list[str]:
    errors: list[str] = []
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    desktop_cargo = tomllib.loads(
        (ROOT / "apps/desktop/src-tauri/Cargo.toml").read_text(encoding="utf-8")
    )
    package = json.loads(
        (ROOT / "apps/desktop/package.json").read_text(encoding="utf-8")
    )
    tauri = json.loads(
        (ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8")
    )
    evidence_schema = json.loads(
        (ROOT / "schemas/windows-release-evidence-v1.json").read_text(
            encoding="utf-8"
        )
    )
    license_policy = json.loads(
        (ROOT / "config/distribution-license-policy-v1.json").read_text(
            encoding="utf-8"
        )
    )
    application_versions = {
        cargo["workspace"]["package"]["version"],
        desktop_cargo["package"]["version"],
        package["version"],
    }
    if application_versions != {VERSION}:
        errors.append(
            f"application versions diverge: {sorted(application_versions)}"
        )
    if tauri["version"] != INSTALLER_VERSION:
        errors.append(
            f"installer version must be MSI-compatible {INSTALLER_VERSION}"
        )
    bundle = tauri.get("bundle", {})
    if not bundle.get("active") or set(bundle.get("targets", [])) != {"msi", "nsis"}:
        errors.append("MSI and NSIS bundle targets must both be active")
    resources = set(bundle.get("resources", []))
    if not {"resources/README.txt", "resources/story-sidecar"} <= resources:
        errors.append("bundled sidecar resources are not declared")
    nsis = bundle.get("windows", {}).get("nsis", {})
    if nsis.get("installerHooks") != "windows/nsis-hooks.nsh":
        errors.append("NSIS WebView2 loader hook is not declared")
    for relative in (
        "rust-toolchain.toml",
        ".nvmrc",
        ".python-version",
        "scripts/build_windows_sidecar.ps1",
        "scripts/verify_pinned_python_dependency.py",
        "scripts/build_windows_release.ps1",
        "scripts/build_signed_windows_release.ps1",
        "scripts/verify_windows_bundle.ps1",
        "apps/desktop/src-tauri/windows/nsis-hooks.nsh",
        "apps/desktop/src-tauri/resources/story-sidecar/README.txt",
        "docs/OPERATOR_GUIDE.md",
        "docs/UPGRADE_ROLLBACK_POLICY.md",
        "docs/SECURITY_REVIEW.md",
        "docs/INCIDENT_RUNBOOK.md",
        "schemas/windows-release-evidence-v1.json",
        "schemas/distribution-license-policy-v1.json",
        "config/distribution-license-policy-v1.json",
        "third_party/licenses/README.md",
        "schemas/provider-health-v1.json",
    ):
        if not (ROOT / relative).is_file():
            errors.append(f"missing release asset: {relative}")
    release_script = (ROOT / "scripts/build_windows_release.ps1").read_text(
        encoding="utf-8"
    )
    sidecar_script = (ROOT / "scripts/build_windows_sidecar.ps1").read_text(
        encoding="utf-8"
    )
    workflow = (ROOT / ".github/workflows/governance.yml").read_text(
        encoding="utf-8"
    )
    for required in (
        "signtool.exe",
        "verify /pa",
        "Find-SignTool",
        "https://timestamp.digicert.com",
        "Get-FileHash",
        "signed = $signed",
        "signing = $signing",
        "Release builds require a clean worktree",
        "source_diff_sha256",
        "ls-files --others --exclude-standard",
        "build_dependency_inventory.py",
        "AllowUnlicensedForLocalVerification",
        "Distribution license review is unresolved",
        "installer_release_eligible",
        "license_policy_sha256",
        "Rust toolchain must be 1.88.0",
        "Refusing to clean bundle output",
        "verify_windows_bundle.ps1",
        "bundle_smoke = $bundleSmoke",
        "Bundled sidecar is missing story-package-v1.json",
        "[IO.File]::WriteAllText",
    ):
        if required not in release_script:
            errors.append(f"release script lacks: {required}")
    for required in (
        'child.Name -eq "README.txt"',
        "expected directory",
        "--force-reinstall",
        "--verify-installed",
        "--add-data",
        "Packaged sidecar is missing story-package-v1.json",
    ):
        if required not in sidecar_script:
            errors.append(f"sidecar build script lacks: {required}")
    signed_script = (
        ROOT / "scripts/build_signed_windows_release.ps1"
    ).read_text(encoding="utf-8")
    for required in (
        "WINDOWS_SIGNING_PFX_BASE64",
        "WINDOWS_SIGNING_PFX_PASSWORD",
        "Import-PfxCertificate",
        "EphemeralKeySet",
        "already exists in CurrentUser\\My",
        "Cert:\\CurrentUser\\My",
        "finally",
        "Remove-Item -LiteralPath $certificatePath",
        "Remove-Item -LiteralPath $pfxPath",
    ):
        if required not in signed_script:
            errors.append(f"signed release script lacks: {required}")
    for required in (
        "windows-release-smoke:",
        "github.event_name == 'workflow_dispatch'",
        r".\scripts\build_windows_release.ps1",
        "AllowUnlicensedForLocalVerification",
        r".\scripts\build_signed_windows_release.ps1",
        "WINDOWS_SIGNING_PFX_BASE64",
        "WINDOWS_SIGNING_PFX_PASSWORD",
        "target/release-evidence/*.json",
    ):
        if required not in workflow:
            errors.append(f"release workflow lacks: {required}")
    schema_required = set(evidence_schema.get("required", []))
    if "bundle_smoke" not in schema_required:
        errors.append("release evidence does not require bundle_smoke")
    if "signing" not in schema_required:
        errors.append("release evidence does not require signing provenance")
    if "distribution_license" not in schema_required:
        errors.append("release evidence does not require license provenance")
    if "installer_release_eligible" not in schema_required:
        errors.append("release evidence does not require installer eligibility")
    smoke_properties = (
        evidence_schema.get("properties", {})
        .get("bundle_smoke", {})
        .get("properties", {})
    )
    for result in (
        "msi_admin_extract",
        "sidecar_protocol",
        "webview2_loader_present",
        "story_schema_present",
        "desktop_launch",
    ):
        if smoke_properties.get(result, {}).get("const") is not True:
            errors.append(f"bundle smoke evidence must require {result}=true")
    policy_entries = {
        (entry.get("ecosystem"), entry.get("name")): entry
        for entry in license_policy.get("dependencies", [])
    }
    if (
        license_policy.get("schema") != "distribution-license-policy/v1"
        or len(policy_entries) != 2
    ):
        errors.append("distribution license policy shape is invalid")
    else:
        campaign_policy = policy_entries.get(("python", "campaign-muti-agent"), {})
        if (
            campaign_policy.get("revision")
            != "1d935714449d18cad5bdc6711a498297ed73a5fb"
            or campaign_policy.get("approved_for_distribution") is not True
            or campaign_policy.get("license") != "MIT"
            or campaign_policy.get("evidence_path")
            != "third_party/licenses/campaign-muti-agent/LICENSE"
        ):
            errors.append("Campaign distribution license policy is inconsistent")
        css_value_policy = policy_entries.get(("npm", "css-value"), {})
        if (
            css_value_policy.get("version") != "0.0.1"
            or css_value_policy.get("integrity")
            != "sha512-FUV3xaJ63buRLgHrLQVlVgQnQdR4yqdLGaDu7g8CQcWjInDfM9plBTPI9FRfpahju1UBSaMckeb2/46ApS/V1Q=="
            or css_value_policy.get("approved_for_distribution") is not True
            or css_value_policy.get("license") != "MIT"
            or css_value_policy.get("evidence_path")
            != "third_party/licenses/css-value-0.0.1/Readme.md"
        ):
            errors.append("css-value distribution license policy is inconsistent")
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
