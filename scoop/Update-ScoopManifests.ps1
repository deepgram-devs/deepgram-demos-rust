[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$Commit
)

$ErrorActionPreference = 'Stop'
$bucketDirectory = Join-Path $PSScriptRoot 'bucket'
$archiveUrl = "https://github.com/deepgram-devs/deepgram-demos-rust/archive/$Commit.zip"
$temporaryArchive = Join-Path $env:TEMP "deepgram-demos-rust-$Commit.zip"

try {
    Invoke-WebRequest -Uri $archiveUrl -OutFile $temporaryArchive
    $hash = (Get-FileHash -LiteralPath $temporaryArchive -Algorithm SHA256).Hash.ToLowerInvariant()

    Get-ChildItem -LiteralPath $bucketDirectory -Filter '*.json' | ForEach-Object {
        $manifest = Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json
        $manifest.url = $archiveUrl
        $manifest.hash = $hash
        $manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $_.FullName -Encoding utf8
    }

    Write-Host "Updated $((Get-ChildItem -LiteralPath $bucketDirectory -Filter '*.json').Count) manifests to $Commit ($hash)."
}
finally {
    Remove-Item -LiteralPath $temporaryArchive -Force -ErrorAction SilentlyContinue
}
