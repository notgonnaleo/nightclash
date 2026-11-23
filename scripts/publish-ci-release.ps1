# Upload all verified assets to a draft, then publish atomically as the latest stable update.
param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')][string]$Repo,
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{40}$')][string]$Commit,
    [string]$Directory = (Join-Path (Split-Path $PSScriptRoot -Parent) 'dist/release'),
    [switch]$Draft,
    [switch]$ValidateOnly
)
$ErrorActionPreference = 'Stop'
$tag = "v$Version"
$manifestPath = Join-Path $Directory 'release.json'
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schema -ne 1 -or $manifest.version -ne $Version -or $manifest.tag -ne $tag) { throw 'Release manifest version/schema mismatch.' }
foreach ($asset in @($manifest.zip, $manifest.launcher)) {
    if (-not $asset -or (Split-Path $asset.name -Leaf) -ne $asset.name) { throw 'Invalid release asset name.' }
    $path = Join-Path $Directory $asset.name
    if ((Get-Item -LiteralPath $path).Length -ne $asset.size -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $asset.sha256) { throw "Release asset verification failed: $($asset.name)" }
}
if ($ValidateOnly) { Write-Output "Verified complete release $tag; nothing uploaded."; exit 0 }
if (-not $env:GH_TOKEN) { throw "GH_TOKEN is empty: it needs Contents: read and write on $Repo; see .claude/RELEASE.md." }
$headers = @{ Authorization = "Bearer $env:GH_TOKEN"; Accept = 'application/vnd.github+json'; 'X-GitHub-Api-Version' = '2022-11-28' }
$api = "https://api.github.com/repos/$Repo"
# Players' launchers read this repository without a token: a private one would lock every launcher out of the update.
try { Invoke-RestMethod $api -Headers @{ Accept = 'application/vnd.github+json' } | Out-Null }
catch { throw "$Repo is not publicly readable: launchers could not download this release. Make it public; see .claude/RELEASE.md." }
$repoInfo = Invoke-RestMethod $api -Headers $headers
# The tag goes on the source commit when the repository has it (the source repository); otherwise (the legacy releases
# repository) on its default branch, which needs at least one commit.
$target = $Commit
try { Invoke-RestMethod "$api/commits/$Commit" -Headers $headers | Out-Null }
catch {
    $target = $repoInfo.default_branch
    try { Invoke-RestMethod "$api/commits/$target" -Headers $headers | Out-Null }
    catch { throw "$Repo has no commits on $target. Push a README there once so releases can be tagged." }
}
$sourceLine = "Built from source commit $Commit."
$releases = @(Invoke-RestMethod "$api/releases?per_page=100" -Headers $headers)
$matching = @($releases | Where-Object tag_name -eq $tag)
if ($matching.Count -gt 1) { throw "Multiple releases claim $tag. Resolve the duplicate drafts before publishing." }
$release = $matching | Select-Object -First 1
if ($release) {
    if (-not $release.draft -or -not "$($release.body)".Contains($sourceLine)) { throw 'This version is published or was built from another commit. Choose a new version.' }
} else {
    $body = @{tag_name=$tag;target_commitish=$target;name="Nightreign PvP $tag";body="$($manifest.notes)`n`n$sourceLine";draft=$true;prerelease=$false} | ConvertTo-Json -Depth 4
    $release = Invoke-RestMethod "$api/releases" -Method Post -Headers $headers -ContentType 'application/json' -Body $body
}
foreach ($name in @($manifest.zip.name, $manifest.launcher.name, 'release.json')) {
    $path = Join-Path $Directory $name
    $existing = $release.assets | Where-Object name -eq $name
    foreach ($asset in $existing) { Invoke-RestMethod "$api/releases/assets/$($asset.id)" -Method Delete -Headers $headers | Out-Null }
    $encodedName = [Uri]::EscapeDataString($name)
    $uploaded = Invoke-RestMethod "https://uploads.github.com/repos/$Repo/releases/$($release.id)/assets?name=$encodedName" -Method Post -Headers $headers -InFile $path -ContentType 'application/octet-stream'
    if ($uploaded.state -ne 'uploaded' -or $uploaded.size -ne (Get-Item -LiteralPath $path).Length) { throw "Incomplete asset upload: $name" }
    if ($uploaded.digest -and $uploaded.digest -ne ('sha256:' + (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant())) { throw "Uploaded checksum mismatch: $name" }
}
if (-not $Draft) {
    $body = @{draft=$false;prerelease=$false;make_latest='true'} | ConvertTo-Json
    $release = Invoke-RestMethod "$api/releases/$($release.id)" -Method Patch -Headers $headers -ContentType 'application/json' -Body $body
}
Write-Output $(if ($Draft) { "Draft ready: $($release.html_url)" } else { "Published update: $($release.html_url)" })
