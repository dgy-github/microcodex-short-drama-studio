param(
    [Parameter(Mandatory = $true)]
    [string]$MsiPath,
    [string]$ResultPath = "",
    [ValidateRange(1, 30)]
    [int]$DesktopAliveSeconds = 5
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$targetRoot = [IO.Path]::GetFullPath((Join-Path $repositoryRoot "target"))
$smokeRoot = Join-Path $targetRoot "windows-bundle-smoke"
$resolvedMsi = (Resolve-Path -LiteralPath $MsiPath).Path

function Assert-PathUnderRoot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Root,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    $fullPath = [IO.Path]::GetFullPath($Path)
    $rootPrefix = [IO.Path]::GetFullPath($Root) +
        [IO.Path]::DirectorySeparatorChar
    if (-not $fullPath.StartsWith(
        $rootPrefix,
        [StringComparison]::OrdinalIgnoreCase
    )) {
        throw "$Description must stay under $Root"
    }
    return $fullPath
}

function Remove-SmokeDirectory {
    if (-not (Test-Path -LiteralPath $smokeRoot)) {
        return
    }
    $resolved = (Resolve-Path -LiteralPath $smokeRoot).Path
    Assert-PathUnderRoot `
        -Path $resolved `
        -Root $targetRoot `
        -Description "Smoke directory" | Out-Null
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            Remove-Item -LiteralPath $resolved -Recurse -Force
            return
        } catch {
            if ($attempt -eq 19) { throw }
            Start-Sleep -Milliseconds 100
        }
    }
}

$resolvedMsi = Assert-PathUnderRoot `
    -Path $resolvedMsi `
    -Root $repositoryRoot `
    -Description "MSI artifact"
if ([IO.Path]::GetExtension($resolvedMsi) -ne ".msi") {
    throw "Bundle smoke requires an MSI artifact"
}
if (-not (Test-Path -LiteralPath $resolvedMsi -PathType Leaf)) {
    throw "MSI artifact is missing: $resolvedMsi"
}

if (-not $ResultPath) {
    $ResultPath = Join-Path $targetRoot `
        "release-evidence\windows-bundle-smoke.json"
} elseif (-not [IO.Path]::IsPathRooted($ResultPath)) {
    $ResultPath = Join-Path $repositoryRoot $ResultPath
}
$resolvedResult = Assert-PathUnderRoot `
    -Path $ResultPath `
    -Root $targetRoot `
    -Description "Smoke result"
$resultDirectory = Split-Path -Parent $resolvedResult
New-Item -ItemType Directory -Path $resultDirectory -Force | Out-Null

$desktopProcess = $null
$sidecarExecutable = $null
$webviewLoader = $null
$storySchema = $null
$previousBundledSidecar = $env:MICROCODEX_TEST_BUNDLED_SIDECAR
$hadBundledSidecar = Test-Path Env:MICROCODEX_TEST_BUNDLED_SIDECAR

try {
    Remove-SmokeDirectory
    New-Item -ItemType Directory -Path $smokeRoot -Force | Out-Null

    $msiexec = Join-Path $env:SystemRoot "System32\msiexec.exe"
    $arguments = @(
        "/a",
        "`"$resolvedMsi`"",
        "/qn",
        "TARGETDIR=`"$smokeRoot`""
    )
    $extract = Start-Process `
        -FilePath $msiexec `
        -ArgumentList $arguments `
        -WindowStyle Hidden `
        -Wait `
        -PassThru
    if ($extract.ExitCode -ne 0) {
        throw "MSI administrative extraction failed: $($extract.ExitCode)"
    }

    $sidecars = @(
        Get-ChildItem -LiteralPath $smokeRoot -Recurse -File `
            -Filter "story-sidecar.exe"
    )
    $desktops = @(
        Get-ChildItem -LiteralPath $smokeRoot -Recurse -File `
            -Filter "story-desktop.exe"
    )
    $webviewLoaders = @(
        Get-ChildItem -LiteralPath $smokeRoot -Recurse -File `
            -Filter "WebView2Loader.dll"
    )
    $storySchemas = @(
        Get-ChildItem -LiteralPath $smokeRoot -Recurse -File `
            -Filter "story-package-v1.json"
    )
    if ($sidecars.Count -ne 1) {
        throw "Expected exactly one extracted story-sidecar.exe"
    }
    if ($desktops.Count -ne 1) {
        throw "Expected exactly one extracted story-desktop.exe"
    }
    if ($webviewLoaders.Count -ne 1) {
        throw "Expected exactly one extracted WebView2Loader.dll"
    }
    if ($storySchemas.Count -ne 1) {
        throw "Expected exactly one extracted story-package-v1.json"
    }
    $sidecarExecutable = $sidecars[0]
    $desktopExecutable = $desktops[0]
    $webviewLoader = $webviewLoaders[0]
    $storySchema = $storySchemas[0]

    $env:MICROCODEX_TEST_BUNDLED_SIDECAR = $sidecarExecutable.FullName
    Push-Location $repositoryRoot
    try {
        cargo test -p story-runtime --test sidecar_process_smoke -- `
            --ignored --nocapture
        if ($LASTEXITCODE -ne 0) {
            throw "Bundled sidecar protocol smoke failed"
        }
    } finally {
        Pop-Location
    }

    $leakedSidecars = @(
        Get-Process -Name "story-sidecar" -ErrorAction SilentlyContinue |
            Where-Object {
                try {
                    $_.Path -eq $sidecarExecutable.FullName
                } catch {
                    $false
                }
            }
    )
    if ($leakedSidecars.Count -ne 0) {
        throw "Bundled sidecar protocol smoke left a child process behind"
    }

    $desktopProcess = Start-Process `
        -FilePath $desktopExecutable.FullName `
        -WorkingDirectory $desktopExecutable.DirectoryName `
        -WindowStyle Hidden `
        -PassThru
    Start-Sleep -Seconds $DesktopAliveSeconds
    if ($desktopProcess.HasExited) {
        throw "Extracted desktop exited during launch smoke: $($desktopProcess.ExitCode)"
    }

    $smoke = [ordered]@{
        msi_path = $resolvedMsi.Substring(
            $repositoryRoot.Length + 1
        ).Replace("\", "/")
        msi_admin_extract = $true
        sidecar_protocol = $true
        webview2_loader_present = $true
        story_schema_present = $true
        desktop_launch = $true
        desktop_alive_seconds = $DesktopAliveSeconds
        sidecar_sha256 = (
            Get-FileHash -LiteralPath $sidecarExecutable.FullName `
                -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        desktop_sha256 = (
            Get-FileHash -LiteralPath $desktopExecutable.FullName `
                -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        webview2_loader_sha256 = (
            Get-FileHash -LiteralPath $webviewLoader.FullName `
                -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        story_schema_sha256 = (
            Get-FileHash -LiteralPath $storySchema.FullName `
                -Algorithm SHA256
        ).Hash.ToLowerInvariant()
    }
    $smokeJson = $smoke | ConvertTo-Json -Depth 4
    [IO.File]::WriteAllText(
        $resolvedResult,
        $smokeJson,
        [Text.UTF8Encoding]::new($false)
    )
    Write-Output "Windows bundle smoke passed: $resolvedResult"
} finally {
    if ($null -ne $desktopProcess -and -not $desktopProcess.HasExited) {
        Stop-Process -Id $desktopProcess.Id
        $desktopProcess.WaitForExit()
    }
    if ($null -ne $sidecarExecutable) {
        Get-Process -Name "story-sidecar" -ErrorAction SilentlyContinue |
            Where-Object {
                try {
                    $_.Path -eq $sidecarExecutable.FullName
                } catch {
                    $false
                }
            } |
            Stop-Process -Force
    }
    if ($hadBundledSidecar) {
        $env:MICROCODEX_TEST_BUNDLED_SIDECAR = $previousBundledSidecar
    } else {
        Remove-Item Env:MICROCODEX_TEST_BUNDLED_SIDECAR `
            -ErrorAction SilentlyContinue
    }
    Remove-SmokeDirectory
}
