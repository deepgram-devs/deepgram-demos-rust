[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$bucketDirectory = Join-Path $PSScriptRoot 'bucket'
$manifests = Get-ChildItem -LiteralPath $bucketDirectory -Filter '*.json'

if ($manifests.Count -eq 0) {
    throw "No Scoop manifests found in $bucketDirectory."
}

$first = $null
foreach ($file in $manifests) {
    $manifest = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
    foreach ($property in 'version', 'description', 'homepage', 'license', 'architecture', 'bin') {
        if ([string]::IsNullOrWhiteSpace($manifest.$property)) {
            throw "$($file.Name) is missing '$property'."
        }
    }

    foreach ($architecture in '64bit', 'arm64') {
        $asset = $manifest.architecture.$architecture
        if ([string]::IsNullOrWhiteSpace($asset.url) -or $asset.hash -notmatch '^[0-9a-fA-F]{64}$') {
            throw "$($file.Name) does not have a valid $architecture release asset and SHA256 hash."
        }
    }

    if ($null -eq $first) {
        $first = $manifest.architecture.'64bit'
    }
}

$temporaryArchive = Join-Path $env:TEMP 'deepgram-demos-rust-scoop-verify.zip'
try {
    Invoke-WebRequest -Uri $first.url -OutFile $temporaryArchive
    $actualHash = (Get-FileHash -LiteralPath $temporaryArchive -Algorithm SHA256).Hash
    if ($actualHash -ne $first.hash) {
        throw "Pinned release asset hash mismatch. Expected $($first.hash), received $actualHash."
    }
}
finally {
    Remove-Item -LiteralPath $temporaryArchive -Force -ErrorAction SilentlyContinue
}

Write-Host "Validated $($manifests.Count) Scoop manifests and a pinned release asset."
