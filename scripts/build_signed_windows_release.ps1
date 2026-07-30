param(
    [string]$Python = ".venv\Scripts\python.exe",
    [switch]$AllowDirty,
    [switch]$SkipSidecarBuild
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$pfxBase64 = $env:WINDOWS_SIGNING_PFX_BASE64
$pfxPassword = $env:WINDOWS_SIGNING_PFX_PASSWORD
$pfxPath = $null
$importedCertificates = @()
$probedCertificates = $null
$pfxBytes = $null

try {
    if ([string]::IsNullOrWhiteSpace($pfxBase64)) {
        throw "WINDOWS_SIGNING_PFX_BASE64 is required"
    }
    if ([string]::IsNullOrWhiteSpace($pfxPassword)) {
        throw "WINDOWS_SIGNING_PFX_PASSWORD is required"
    }
    $temporaryRoot = if ($env:RUNNER_TEMP) {
        [IO.Path]::GetFullPath($env:RUNNER_TEMP)
    } else {
        [IO.Path]::GetFullPath(
            (Join-Path $repositoryRoot "target\release-signing")
        )
    }
    New-Item -ItemType Directory -Path $temporaryRoot -Force | Out-Null
    $pfxPath = Join-Path $temporaryRoot `
        "microcodex-signing-$([Guid]::NewGuid().ToString('N')).pfx"
    try {
        $pfxBytes = [Convert]::FromBase64String($pfxBase64)
    } catch {
        throw "WINDOWS_SIGNING_PFX_BASE64 is not valid base64"
    }
    $probedCertificates = [Security.Cryptography.X509Certificates.X509Certificate2Collection]::new()
    try {
        $probedCertificates.Import(
            $pfxBytes,
            $pfxPassword,
            [Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet
        )
    } catch {
        throw "Signing PFX could not be opened with the supplied password"
    }
    $privateKeyCertificates = @(
        $probedCertificates | Where-Object { $_.HasPrivateKey }
    )
    if ($privateKeyCertificates.Count -ne 1) {
        throw "Signing PFX must contain exactly one private-key certificate"
    }
    $candidateThumbprint = $privateKeyCertificates[0].Thumbprint
    $existingCertificatePath = "Cert:\CurrentUser\My\$candidateThumbprint"
    if (Test-Path -LiteralPath $existingCertificatePath -PathType Leaf) {
        throw "Signing certificate already exists in CurrentUser\My"
    }

    [IO.File]::WriteAllBytes($pfxPath, $pfxBytes)
    $securePassword = ConvertTo-SecureString `
        -String $pfxPassword `
        -AsPlainText `
        -Force
    $importedCertificates = @(
        Import-PfxCertificate `
            -FilePath $pfxPath `
            -CertStoreLocation "Cert:\CurrentUser\My" `
            -Password $securePassword
    )
    if ($importedCertificates.Count -ne 1) {
        throw "Signing PFX must import exactly one certificate"
    }
    $certificate = $importedCertificates[0]
    $hasCodeSigningEku = @(
        $certificate.EnhancedKeyUsageList |
            Where-Object { $_.ObjectId.Value -eq "1.3.6.1.5.5.7.3.3" }
    ).Count -gt 0
    $now = Get-Date
    if (-not $certificate.HasPrivateKey) {
        throw "Imported signing certificate has no private key"
    }
    if (-not $hasCodeSigningEku) {
        throw "Imported certificate is not valid for code signing"
    }
    if ($now -lt $certificate.NotBefore -or $now -gt $certificate.NotAfter) {
        throw "Imported signing certificate is not currently valid"
    }

    $releaseParameters = @{
        CertificateThumbprint = $certificate.Thumbprint
        Python = $Python
    }
    if ($AllowDirty) {
        $releaseParameters.AllowDirty = $true
    }
    if ($SkipSidecarBuild) {
        $releaseParameters.SkipSidecarBuild = $true
    }
    & (Join-Path $PSScriptRoot "build_windows_release.ps1") `
        @releaseParameters
    if ($LASTEXITCODE -ne 0) {
        throw "Signed Windows release build failed"
    }
} finally {
    foreach ($certificate in $importedCertificates) {
        $certificatePath = "Cert:\CurrentUser\My\$($certificate.Thumbprint)"
        if (Test-Path -LiteralPath $certificatePath -PathType Leaf) {
            Remove-Item -LiteralPath $certificatePath -Force
        }
    }
    if ($pfxPath -and (Test-Path -LiteralPath $pfxPath -PathType Leaf)) {
        Remove-Item -LiteralPath $pfxPath -Force
    }
    if ($null -ne $pfxBytes) {
        [Array]::Clear($pfxBytes, 0, $pfxBytes.Length)
    }
    if ($null -ne $probedCertificates) {
        foreach ($certificate in $probedCertificates) {
            $certificate.Dispose()
        }
    }
    $env:WINDOWS_SIGNING_PFX_BASE64 = $null
    $env:WINDOWS_SIGNING_PFX_PASSWORD = $null
}
