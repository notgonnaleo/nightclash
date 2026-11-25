# Prepare this PC for an `nro` run with four emulated instances (scenarios\mi-arena-duo.toml), or put it back afterwards.
# The settings are the ones of the first 4-instance run (2026-11-24, .claude/testing-log/2026-11-24-arena-duo.md) on 32 GB RAM:
#   -Prepare  closes Steam (an emulated run needs it closed), Discord and WhatsApp; saves %APPDATA%\Nightreign\GraphicsConfig.xml
#             once as GraphicsConfig.xml.before-4inst and sets it to WINDOW 960x540 with every quality LOW (the file is shared by
#             every instance AND the real game).
#   -Restore  puts the saved GraphicsConfig.xml back and deletes the copy (stop the games first: `nro stop --all`).
# Needs once: instance 4's save (play the new-game start on `nro launch --emulated 4`) and its firewall rule (elevated):
#   New-NetFirewallRule -DisplayName "nro i4 (block inbound)" -Direction Inbound -Action Block -Program "Z:\nro-instances\i4\game\nightreign.exe"
param([switch]$Prepare, [switch]$Restore)
$ErrorActionPreference = 'Stop'

$cfg = Join-Path $env:APPDATA 'Nightreign\GraphicsConfig.xml'
$bak = "$cfg.before-4inst"
# The game writes the file as UTF-16 LE with no byte-order mark: read and write bytes, never ReadAllText (it guesses UTF-8).
$utf16 = [System.Text.Encoding]::Unicode

if ($Prepare -eq $Restore) { throw 'Pass exactly one of -Prepare or -Restore.' }

if ($Restore) {
    if (Get-Process nightreign -ErrorAction SilentlyContinue) { throw 'A game is still running: run `nro stop --all` first.' }
    if (-not (Test-Path $bak)) { Write-Host "Nothing to restore: $bak does not exist."; exit 0 }
    Copy-Item $bak $cfg -Force
    Remove-Item $bak -Confirm:$false
    Write-Host "Restored $cfg from the saved copy."
    exit 0
}

# -Prepare
if (Test-Path $bak) {
    Write-Host "Keeping the existing saved copy $bak (it holds the settings from before the first -Prepare)."
} else {
    Copy-Item $cfg $bak
    Write-Host "Saved $cfg as $bak."
}
$x = $utf16.GetString([System.IO.File]::ReadAllBytes($bak))
$x = $x -replace '<ScreenMode>[^<]*<', '<ScreenMode>WINDOW<' `
    -replace '<Resolution-WindowScreenWidth>[^<]*<', '<Resolution-WindowScreenWidth>960<' `
    -replace '<Resolution-WindowScreenHeight>[^<]*<', '<Resolution-WindowScreenHeight>540<'
$low = 'TextureQuality', 'Antialiasing', 'SSAO', 'DepthOfField', 'MotionBlur', 'ShadowQuality', 'LightingQuality', 'EffectsQuality',
    'ReflectionQuality', 'WaterSurfaceQuality', 'ShadeQuality', 'VolumetricEffectQuality', 'GIDataQuality', 'GrassQuality'
foreach ($k in $low) { $x = $x -replace "<$k>[^<]*<", "<$k>LOW<" }
if ($x -notmatch '<ScreenMode>WINDOW<' -or $x -notmatch '<TextureQuality>LOW<') { throw "GraphicsConfig.xml did not take the low settings (format changed?)" }
[System.IO.File]::WriteAllBytes($cfg, $utf16.GetBytes($x))
Write-Host 'Graphics: WINDOW 960x540, every quality LOW.'

$steam = Get-Process steam -ErrorAction SilentlyContinue | Select-Object -First 1
if ($steam) {
    Start-Process -FilePath $steam.Path -ArgumentList '-shutdown'
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Process steam -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    Write-Host 'Steam closed.'
}
foreach ($n in 'Discord', 'WhatsApp', 'WhatsApp.Root') {
    try { Get-Process $n -ErrorAction Stop | Stop-Process -Force -Confirm:$false -ErrorAction Stop; Write-Host "$n closed." } catch {}
}
$os = Get-CimInstance Win32_OperatingSystem
Write-Host ('Free RAM {0:N1} GB (about 18 GB or more is what the first run started with).' -f ($os.FreePhysicalMemory / 1MB))
Write-Host 'Next: `nro desk` in a normal terminal, then `nro run scenarios\mi-arena-duo.toml`; hands off the PC while it runs.'
