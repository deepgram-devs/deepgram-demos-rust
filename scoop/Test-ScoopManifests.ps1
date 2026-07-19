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
    foreach ($property in 'version', 'description', 'homepage', 'license', 'url', 'hash', 'bin') {
        if ([string]::IsNullOrWhiteSpace($manifest.$property)) {
            throw "$($file.Name) is missing '$property'."
        }
    }

    if ($manifest.hash -notmatch '^[0-9a-fA-F]{64}$') {
        throw "$($file.Name) does not have a SHA256 hash."
    }

    if ($null -eq $first) {
        $first = $manifest
    }
}

$temporaryArchive = Join-Path $env:TEMP 'deepgram-demos-rust-scoop-verify.zip'
try {
    Invoke-WebRequest -Uri $first.url -OutFile $temporaryArchive
    $actualHash = (Get-FileHash -LiteralPath $temporaryArchive -Algorithm SHA256).Hash
    if ($actualHash -ne $first.hash) {
        throw "Pinned archive hash mismatch. Expected $($first.hash), received $actualHash."
    }
}
finally {
    Remove-Item -LiteralPath $temporaryArchive -Force -ErrorAction SilentlyContinue
}

Write-Host "Validated $($manifests.Count) Scoop manifests and the pinned source archive."
