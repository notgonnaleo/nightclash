---
name: release-zip
description: Build and verify the player zip dist\NightreignPvP.zip (or the Nucleus/dev deploy) for nightreign-pvp - DLL, game-data package, launcher, bundled me3, settings, VERSION.txt - and record what was built. Use before any friend session, when Leonardo asks for a new zip, or after a change that players must get.
---

# Release zip

Every friend session fails silently if the two PCs run different zips, so the zip is built the same way every time and its identity
is written down.

## 1. Preconditions

- Game, Seamless Co-op and me3 match the pins in `TECHNICAL_NOTES.md` §1. If the game updated, stop: offsets and the game-data pins
  must be re-verified first.
- No game is running from `dist\` (the build cannot replace a DLL in use).
- `scripts\regress.ps1` is all PASS when the change touches night flow, hold/release, downed rules or the console (one PC, needs `nro desk`; TOOLS.md).
- `cargo test --workspace` passes. Note whether the tree is dirty: VERSION.txt records it.

## 2. Build (repo root, PowerShell)

1. `powershell -NoProfile -File scripts\build.ps1`: builds `dist\nrpvp.dll` (+ `.pdb`).
2. If `gamedata\*.json` changed or `dist\gamedata` is missing: `powershell -ExecutionPolicy Bypass -File scripts\build-gamedata.ps1`
   (needs WitchyBND and the 1.03.3 unpack in `Z:\Tools\`, and Python).
3. `powershell -ExecutionPolicy Bypass -File scripts\package-release.ps1 -MatchPlayers 2`. It builds the **player** DLL itself
   (`--no-default-features`: no developer tooling) into `target\player\`, builds the launcher and packs. Use `-MatchPlayers 6`
   only for the final game, `-TeamSize N` for team tests. Everyone who should meet needs the same `-MatchPlayers`.

For Nucleus instead: `powershell -ExecutionPolicy Bypass -File scripts\deploy-nucleus.ps1 [-TeamSize N]` (close Nucleus first; it
rewrites the handler's settings, logs go to `dist\nucleus-logs\`).

## 3. Verify the zip before handing it over

- The script prints `OK: ...zip (N MB)` and the two VERSION lines: `version local` (or `version X.Y.Z` with `-Version`; the
  launcher's version gate never blocks `local`, RELEASE.md §5c), then build time, commit, clean/dirty, `nrpvp.dll <hash16>`, gamedata hash.
- The zip holds `nrpvp_launcher.exe`, `HOW TO INSTALL.txt`, and `NightreignPvP\` with `nrpvp.dll`, `nrpvp.toml`, `VERSION.txt`,
  `gamedata\` (17 files: 15 languages + the Roundtable event script + the hawk crossing line `map\mapstudio\m60_00_00_99.msb.dcx`, HAWK-2;
  since BOT-5 (2026-11-06) `event\common.emevd.dcx` with the bot events goes into the zip as it is (players use Bots On);
  the VERSION line names its hash as `hawk routes`), `me3\` (3 binaries + 2 licences) and `LICENSE.txt` (GPL-3.0, from the repo's `LICENSE`). The script refuses an incomplete
  `gamedata`; still list the archive once.
- `nrpvp.toml` in the zip has no `dev_mode = true` (a player DLL would ignore it anyway, with a warning).
- Never install the zip into Leonardo's game folder yourself; installing is his step (STATUS §2 notes this each time).

## 4. Record it

- **STATUS.md §2**: the zip's date, `nrpvp.dll` hash, flags (`-MatchPlayers`, `-TeamSize`), and that it is not installed yet.
- **TESTING.md §6**: the runbook for the session points at this zip (date + hash), so both players can check `VERSION.txt` matches.
- Old zips: say which one must not be sent (as STATUS does for `friend-kit.zip`).
