param(
    [string]$CertificateThumbprint = "",
    [string]$Python = ".venv\Scripts\python.exe",
    [switch]$AllowDirty,
    [switch]$AllowUnlicensedForLocalVerification,
    [switch]$SkipSidecarBuild
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$desktopRoot = Join-Path $repositoryRoot "apps\desktop"
$releaseRoot = Join-Path $repositoryRoot "target\release-evidence"
$bundleRoot = Join-Path $desktopRoot "src-tauri\target\release\bundle"

function Get-SourceStateHash {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Root,
        [Parameter(Mandatory = $true)]
        [object]$Status
    )

    $trackedDiff = git -C $Root diff --binary --no-ext-diff HEAD | Out-String
    if ($LASTEXITCODE -ne 0) { throw "Unable to read tracked source diff" }
    $untracked = @(
        git -C $Root ls-files --others --exclude-standard |
            Sort-Object
    )
    if ($LASTEXITCODE -ne 0) { throw "Unable to enumerate untracked source files" }
    $state = [Text.StringBuilder]::new()
    [void]$state.AppendLine("status")
    foreach ($line in @($Status | Sort-Object)) {
        [void]$state.AppendLine($line)
    }
    [void]$state.AppendLine("tracked-diff")
    [void]$state.Append($trackedDiff)
    [void]$state.AppendLine("untracked")
    foreach ($relative in $untracked) {
        $fullPath = Join-Path $Root $relative
        if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
            throw "Untracked source is not a regular file: $relative"
        }
        $file = Get-Item -LiteralPath $fullPath
        $fileHash = (
            Get-FileHash -LiteralPath $fullPath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        [void]$state.AppendLine(
            "$($relative.Replace('\', '/'))`0$($file.Length)`0$fileHash"
        )
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes($state.ToString())
    $hasher = [Security.Cryptography.SHA256]::Create()
    try {
        return -join (
            $hasher.ComputeHash($bytes) |
                ForEach-Object { $_.ToString("x2") }
        )
    } finally {
        $hasher.Dispose()
    }
}

function Find-SignTool {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $kitRoots = @()
    foreach ($registryPath in @(
        "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots",
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows Kits\Installed Roots"
    )) {
        $installedRoots = Get-ItemProperty `
            -LiteralPath $registryPath `
            -ErrorAction SilentlyContinue
        if ($installedRoots.KitsRoot10) {
            $kitRoots += $installedRoots.KitsRoot10
        }
    }
    $kitRoots += "C:\Program Files (x86)\Windows Kits\10"
    $candidates = @(
        foreach ($root in $kitRoots | Select-Object -Unique) {
            $bin = Join-Path $root "bin"
            if (Test-Path -LiteralPath $bin -PathType Container) {
                Get-ChildItem -LiteralPath $bin -Recurse -File `
                    -Filter "signtool.exe" -ErrorAction SilentlyContinue |
                    Where-Object {
                        $_.FullName -match "\\x64\\signtool\.exe$"
                    }
            }
        }
    )
    $selected = $candidates | Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $selected) {
        throw "signtool.exe was not found on PATH or in the Windows 10 SDK"
    }
    return $selected.FullName
}

function Get-CodeSigningCertificate {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Thumbprint
    )

    $normalized = $Thumbprint.Replace(" ", "").ToUpperInvariant()
    if ($normalized -notmatch "^[A-F0-9]{40}$") {
        throw "Certificate thumbprint must contain exactly 40 hexadecimal characters"
    }
    $certificatePath = "Cert:\CurrentUser\My\$normalized"
    if (-not (Test-Path -LiteralPath $certificatePath -PathType Leaf)) {
        throw "Code-signing certificate is not in CurrentUser\My"
    }
    $certificate = Get-Item -LiteralPath $certificatePath
    $now = Get-Date
    if (-not $certificate.HasPrivateKey) {
        throw "Code-signing certificate has no private key"
    }
    if ($now -lt $certificate.NotBefore -or $now -gt $certificate.NotAfter) {
        throw "Code-signing certificate is not currently valid"
    }
    $hasCodeSigningEku = @(
        $certificate.EnhancedKeyUsageList |
            Where-Object { $_.ObjectId.Value -eq "1.3.6.1.5.5.7.3.3" }
    ).Count -gt 0
    if (-not $hasCodeSigningEku) {
        throw "Certificate is not valid for code signing"
    }
    return $certificate
}

$dirtyStatus = git -C $repositoryRoot status --porcelain
if ($dirtyStatus -and -not $AllowDirty) {
    throw "Release builds require a clean worktree. Use -AllowDirty only for local pipeline verification."
}
$diffHash = Get-SourceStateHash -Root $repositoryRoot -Status $dirtyStatus

$requiredRust = "rustc 1.88.0 "
$requiredNode = "v22.14.0"
$requiredPython = "Python 3.12.10"
$rust = (rustc --version).Trim()
$node = (node --version).Trim()
$pythonPath = Join-Path $repositoryRoot $Python
$pythonVersion = (& $pythonPath --version).Trim()
if (-not $rust.StartsWith($requiredRust)) { throw "Rust toolchain must be 1.88.0" }
if ($node -ne $requiredNode) { throw "Node toolchain must be $requiredNode" }
if ($pythonVersion -ne $requiredPython) { throw "Python toolchain must be $requiredPython" }

if ($AllowUnlicensedForLocalVerification -and -not $AllowDirty) {
    throw "The unlicensed override requires -AllowDirty local verification mode"
}
if ($AllowUnlicensedForLocalVerification -and $CertificateThumbprint) {
    throw "Unlicensed local verification cannot be signed"
}
New-Item -ItemType Directory -Path $releaseRoot -Force | Out-Null
$inventoryPath = Join-Path $releaseRoot "dependency-inventory.json"
& $pythonPath (Join-Path $PSScriptRoot "build_dependency_inventory.py") `
    --output $inventoryPath
if ($LASTEXITCODE -ne 0) { throw "Dependency license inventory failed" }
$licenseInventory = Get-Content `
    -LiteralPath $inventoryPath `
    -Raw `
    -Encoding utf8 | ConvertFrom-Json
$licenseReviewRequired = @($licenseInventory.review_required)
$distributionLicensed = [bool]$licenseInventory.distribution_cleared
if (-not $distributionLicensed -and -not $AllowUnlicensedForLocalVerification) {
    $unresolved = $licenseReviewRequired -join ", "
    throw "Distribution license review is unresolved: $unresolved"
}

if (-not $SkipSidecarBuild) {
    & (Join-Path $PSScriptRoot "build_windows_sidecar.ps1") -Python $Python
    if ($LASTEXITCODE -ne 0) { throw "Sidecar build failed" }
}
$bundledSidecar = Join-Path $repositoryRoot `
    "apps\desktop\src-tauri\resources\story-sidecar\story-sidecar.exe"
if (-not (Test-Path -LiteralPath $bundledSidecar -PathType Leaf)) {
    throw "Bundled sidecar is missing"
}
$bundledStorySchema = Join-Path $repositoryRoot `
    "apps\desktop\src-tauri\resources\story-sidecar\_internal\schemas\story-package-v1.json"
if (-not (Test-Path -LiteralPath $bundledStorySchema -PathType Leaf)) {
    throw "Bundled sidecar is missing story-package-v1.json"
}

if (Test-Path -LiteralPath $bundleRoot) {
    $resolvedBundleRoot = (Resolve-Path -LiteralPath $bundleRoot).Path
    $expectedTargetRoot = [IO.Path]::GetFullPath(
        (Join-Path $desktopRoot "src-tauri\target")
    ) + [IO.Path]::DirectorySeparatorChar
    if (-not $resolvedBundleRoot.StartsWith($expectedTargetRoot)) {
        throw "Refusing to clean bundle output outside the desktop target directory"
    }
    Remove-Item -LiteralPath $resolvedBundleRoot -Recurse -Force
}

Push-Location $desktopRoot
try {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw "npm ci failed" }
    npm run check
    if ($LASTEXITCODE -ne 0) { throw "Svelte check failed" }
    npm run tauri build
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }
} finally {
    Pop-Location
}

$wixSource = Join-Path $desktopRoot "src-tauri\target\release\wix\x64\main.wxs"
$nsisSource = Join-Path $desktopRoot "src-tauri\target\release\nsis\x64\installer.nsi"
foreach ($source in @($wixSource, $nsisSource)) {
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Generated installer source is missing: $source"
    }
    if (-not (Select-String -LiteralPath $source -SimpleMatch "story-sidecar.exe" -Quiet)) {
        throw "Installer does not contain the bundled sidecar: $source"
    }
}
$nsisHooks = Join-Path $desktopRoot "src-tauri\windows\nsis-hooks.nsh"
if (-not (
    (Select-String -LiteralPath $wixSource -SimpleMatch "WebView2Loader.dll" -Quiet) -and
    (Select-String -LiteralPath $nsisSource -SimpleMatch "nsis-hooks.nsh" -Quiet) -and
    (Select-String -LiteralPath $nsisHooks -SimpleMatch "WebView2Loader.dll" -Quiet)
)) {
    throw "MSI or NSIS does not contain WebView2Loader.dll"
}

$bundles = Get-ChildItem -LiteralPath $bundleRoot `
    -File -Recurse | Where-Object { $_.Extension -in ".msi", ".exe" }
if (-not $bundles) { throw "No MSI or NSIS release bundle was produced" }
$msiBundles = @($bundles | Where-Object { $_.Extension -eq ".msi" })
if ($msiBundles.Count -ne 1) {
    throw "Expected exactly one MSI release bundle"
}

$smokeResultPath = Join-Path $releaseRoot "windows-bundle-smoke.json"
& (Join-Path $PSScriptRoot "verify_windows_bundle.ps1") `
    -MsiPath $msiBundles[0].FullName `
    -ResultPath $smokeResultPath
if ($LASTEXITCODE -ne 0) { throw "Windows bundle smoke failed" }
$bundleSmoke = Get-Content -LiteralPath $smokeResultPath -Raw -Encoding utf8 |
    ConvertFrom-Json

$signed = $false
$signing = $null
if ($CertificateThumbprint) {
    $certificate = Get-CodeSigningCertificate `
        -Thumbprint $CertificateThumbprint
    $signTool = Find-SignTool
    $timestampUrl = "https://timestamp.digicert.com"
    foreach ($bundle in $bundles) {
        & $signTool sign /s My /sha1 $certificate.Thumbprint /fd SHA256 /tr `
            $timestampUrl /td SHA256 $bundle.FullName
        if ($LASTEXITCODE -ne 0) { throw "Signing failed: $($bundle.FullName)" }
        & $signTool verify /pa $bundle.FullName
        if ($LASTEXITCODE -ne 0) { throw "Signature verification failed" }
    }
    $signed = $true
    $signing = [ordered]@{
        certificate_thumbprint = $certificate.Thumbprint.ToUpperInvariant()
        timestamp_url = $timestampUrl
        verified = $true
    }
}

$commit = (git -C $repositoryRoot rev-parse HEAD).Trim()
$artifacts = @(
    $bundles | ForEach-Object {
        [ordered]@{
            path = $_.FullName.Substring($repositoryRoot.Length + 1).Replace("\", "/")
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            bytes = $_.Length
        }
    }
)
$installerReleaseEligible = (
    -not [bool]$dirtyStatus -and
    $signed -and
    $distributionLicensed
)
$evidence = [ordered]@{
    schema = "windows-release-evidence/v1"
    version = "0.1.0-alpha.1"
    installer_version = "0.1.0-1"
    commit = $commit
    dirty = [bool]$dirtyStatus
    source_diff_sha256 = $diffHash
    tools = [ordered]@{ rust = $rust; node = $node; python = $pythonVersion }
    lockfiles = @("Cargo.lock", "apps/desktop/package-lock.json", "sidecar/pyproject.toml")
    signed = $signed
    signing = $signing
    distribution_license = [ordered]@{
        cleared = $distributionLicensed
        review_required = $licenseReviewRequired
        policy = $licenseInventory.license_policy
        policy_sha256 = $licenseInventory.license_policy_sha256
    }
    installer_release_eligible = $installerReleaseEligible
    bundle_smoke = $bundleSmoke
    artifacts = $artifacts
}
$evidencePath = Join-Path $releaseRoot "windows-release-evidence.json"
$evidenceJson = $evidence | ConvertTo-Json -Depth 6
[IO.File]::WriteAllText(
    $evidencePath,
    $evidenceJson,
    [Text.UTF8Encoding]::new($false)
)
Write-Output "Release evidence: $releaseRoot\windows-release-evidence.json"
if (-not $signed) {
    Write-Warning "Artifacts are unsigned and are not eligible for stable release."
}
if ($dirtyStatus) {
    Write-Warning "Artifacts were built from a dirty worktree and are local verification only."
}
if (-not $distributionLicensed) {
    Write-Warning "Artifacts include dependencies without cleared distribution licenses."
}
