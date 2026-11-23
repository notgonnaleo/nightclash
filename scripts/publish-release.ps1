# Publishes a player release for the launcher app (crates/nrpvp-app): builds the player zip and the launcher exe, writes release.json,
# and uploads all three as a GitHub release of the public source repository (notgonnaleo/nightreign-pvp; RELEASE.md §5b).
# The launcher downloads release.json via the GitHub release asset API; the latest stable release is what players get.
#
#   ./scripts/publish-release.ps1 -Version 0.2.0 -NotesFile notes.txt -DryRun     # build + write dist\release\, upload nothing
#   ./scripts/publish-release.ps1 -Version 0.2.0 -NotesFile notes.txt             # build + upload (needs `gh auth login`)
#
# Notes file: plain text; blank lines separate paragraphs, lines starting with "- " are list items (the launcher shows it as patch notes).
param(
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [Parameter(Mandatory)][string]$NotesFile,
    [ValidateRange(2, 6)][int]$MatchPlayers = 2,
    [ValidateRange(0, 6)][int]$MinPlayers = 0,
    [string]$Me3Dir = 'Z:\Tools\me3',
    # Shown to players; must agree with the hashes in pins/nightreign.toml.
    [string]$GameVersion = '1.03.3',
    [string]$ScVersion = '1.1.3',
    [string]$Repo = 'notgonnaleo/nightreign-pvp',
    [switch]$DryRun,
    [switch]$AllowDirty
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $repoRoot 'dist'
$out = Join-Path $dist 'release'
$tag = "v$Version"

if (-not (Test-Path -LiteralPath $NotesFile)) { throw "Notes file not found: $NotesFile" }
if (-not $AllowDirty -and (& git -C $repoRoot status --porcelain)) { throw 'The working tree has changes. Commit them first (or pass -AllowDirty for a dry run).' }
# Pins: the game exe and Seamless Co-op this release was built and tested with (pins/nightreign.toml, TECHNICAL_NOTES §1).
$pins = Get-Content -LiteralPath (Join-Path $repoRoot 'pins\nightreign.toml') -Raw
function Pin([string]$key) {
    $m = [regex]::Match($pins, '"' + [regex]::Escape($key) + '"\s*=\s*"([0-9A-Fa-f]{64})"')
    if (-not $m.Success) { throw "pins/nightreign.toml has no hash for $key" }
    $m.Groups[1].Value.ToUpper()
}
$gameSha = Pin 'nightreign.exe'
$nrscSha = Pin 'SeamlessCoop/nrsc.dll'

# 1. The player zip.
& (Join-Path $PSScriptRoot 'package-release.ps1') -MatchPlayers $MatchPlayers -MinPlayers $MinPlayers -Me3Dir $Me3Dir -Version $Version
$zip = Join-Path $dist 'NightreignPvP.zip'

# 2. The launcher app.
$cargo = (Get-Command cargo -ErrorAction SilentlyContinue).Source
if (-not $cargo) { $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
$prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
$oldBuildVersion = $env:NRPVP_BUILD_VERSION
$oldReleasesRepo = $env:NRPVP_RELEASES_REPO
$env:NRPVP_BUILD_VERSION = $Version
$env:NRPVP_RELEASES_REPO = $Repo
try { & $cargo build --locked --release -p nrpvp-app --manifest-path (Join-Path $repoRoot 'Cargo.toml') }
finally { $env:NRPVP_BUILD_VERSION = $oldBuildVersion; $env:NRPVP_RELEASES_REPO = $oldReleasesRepo }
$code = $LASTEXITCODE; $ErrorActionPreference = $prev
if ($code -ne 0) { throw "cargo build of the launcher app failed ($code)" }

$expectedOut = [IO.Path]::GetFullPath((Join-Path $repoRoot 'dist\release'))
if ([IO.Path]::GetFullPath($out) -ne $expectedOut) { throw 'Release output is outside the expected dist directory.' }
if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Recurse -Force }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item -LiteralPath $zip -Destination $out
$launcher = Join-Path $out 'NightreignPvP-Launcher.exe'
Copy-Item -LiteralPath (Join-Path $repoRoot 'target\release\NightreignPvP.exe') -Destination $launcher

function Asset([string]$path) {
    [ordered]@{ name = (Split-Path $path -Leaf); sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash; size = (Get-Item -LiteralPath $path).Length }
}
$manifest = [ordered]@{
    schema         = 1
    version        = $Version
    tag            = $tag
    published      = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd')
    notes          = ((Get-Content -LiteralPath $NotesFile -Raw) -replace "`r`n", "`n").Trim()
    zip            = Asset (Join-Path $out 'NightreignPvP.zip')
    launcher       = Asset $launcher
    game           = [ordered]@{ version = $GameVersion; exe_sha256 = $gameSha }
    seamless_coop  = [ordered]@{ version = $ScVersion; nrsc_sha256 = $nrscSha }
    match_players  = $MatchPlayers
    min_players    = $(if ($MinPlayers -ge 2) { [Math]::Min($MinPlayers, $MatchPlayers) } else { $MatchPlayers })
}
$json = Join-Path $out 'release.json'
[IO.File]::WriteAllText($json, ($manifest | ConvertTo-Json -Depth 4), (New-Object Text.UTF8Encoding($false)))
Write-Output "Release $tag staged in $out"
Get-ChildItem $out | ForEach-Object { Write-Output ("  {0,-30} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB)) }

if ($DryRun) {
    Write-Output "Dry run: nothing uploaded. Test it with:  `$env:NRPVP_RELEASE_DIR = '$out'; & '$launcher'"
    return
}
& gh release create $tag -R $Repo --title "Nightreign PvP $tag" --notes-file $NotesFile (Join-Path $out 'NightreignPvP.zip') $launcher $json
if ($LASTEXITCODE -ne 0) { throw "gh release create failed ($LASTEXITCODE)" }
Write-Output "Published: https://github.com/$Repo/releases/tag/$tag"
