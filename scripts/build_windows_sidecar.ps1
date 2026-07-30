param(
    [string]$Python = ".venv\Scripts\python.exe"
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$pythonPath = Join-Path $repositoryRoot $Python
if (-not (Test-Path -LiteralPath $pythonPath -PathType Leaf)) {
    throw "Python executable not found: $pythonPath"
}

& $pythonPath -m pip install --editable (Join-Path $repositoryRoot "sidecar")
if ($LASTEXITCODE -ne 0) { throw "Sidecar dependency installation failed" }
$pinVerifier = Join-Path $repositoryRoot "scripts\verify_pinned_python_dependency.py"
$campaignRequirement = & $pythonPath $pinVerifier --print-requirement
if ($LASTEXITCODE -ne 0 -or -not $campaignRequirement) {
    throw "Unable to resolve the pinned Campaign dependency"
}
& $pythonPath -m pip install --force-reinstall --no-deps --no-cache-dir `
    $campaignRequirement
if ($LASTEXITCODE -ne 0) { throw "Pinned Campaign installation failed" }
& $pythonPath $pinVerifier --verify-installed
if ($LASTEXITCODE -ne 0) { throw "Pinned Campaign verification failed" }
& $pythonPath -m pip install --requirement (Join-Path $repositoryRoot "sidecar\requirements-release.txt")
if ($LASTEXITCODE -ne 0) { throw "PyInstaller dependency installation failed" }

$distribution = Join-Path $repositoryRoot "target\sidecar-dist-onedir"
$work = Join-Path $repositoryRoot "target\sidecar-build"
& $pythonPath -m PyInstaller `
    --noconfirm `
    --clean `
    --onedir `
    --name story-sidecar `
    --paths (Join-Path $repositoryRoot "sidecar") `
    --collect-all campaign `
    --add-data "$repositoryRoot\schemas;schemas" `
    --distpath $distribution `
    --workpath $work `
    --specpath $work `
    (Join-Path $repositoryRoot "sidecar\story_sidecar.py")
if ($LASTEXITCODE -ne 0) { throw "Sidecar packaging failed" }
$packagedSchema = Join-Path $distribution `
    "story-sidecar\_internal\schemas\story-package-v1.json"
if (-not (Test-Path -LiteralPath $packagedSchema -PathType Leaf)) {
    throw "Packaged sidecar is missing story-package-v1.json"
}

$resourceDirectory = Join-Path $repositoryRoot "apps\desktop\src-tauri\resources"
$resourceBundle = Join-Path $resourceDirectory "story-sidecar"
if (Test-Path -LiteralPath $resourceBundle) {
    $resolvedBundle = (Resolve-Path -LiteralPath $resourceBundle).Path
    $expectedRoot = [IO.Path]::GetFullPath($resourceDirectory) +
        [IO.Path]::DirectorySeparatorChar
    if (-not $resolvedBundle.StartsWith($expectedRoot)) {
        throw "Refusing to clean sidecar resources outside the expected directory"
    }
    foreach ($child in Get-ChildItem -LiteralPath $resolvedBundle -Force) {
        if ($child.Name -eq "README.txt") {
            continue
        }
        if (-not $child.FullName.StartsWith(
            $expectedRoot,
            [StringComparison]::OrdinalIgnoreCase
        )) {
            throw "Refusing to clean sidecar resource outside the expected directory"
        }
        Remove-Item -LiteralPath $child.FullName -Recurse -Force
    }
}
New-Item -ItemType Directory -Path $resourceBundle -Force | Out-Null
Copy-Item -Path (Join-Path $distribution "story-sidecar\*") `
    -Destination $resourceBundle -Recurse -Force
Write-Output "Bundled sidecar ready: $resourceBundle\story-sidecar.exe"
