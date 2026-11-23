# How to configure, build and release the mod (player zip)

Owner doc for: the build chain, the settings that matter for a release, the packaging command, and what to hand to the other players.
Short checklist for agents: the `release-zip` skill (`.claude/skills/release-zip/SKILL.md`). Tools overview: [TOOLS.md](TOOLS.md).

The product of a release is **one file, `dist\NightreignPvP.zip`**. Players drop two things into their game folder and start the game with
`nrpvp_launcher.exe`. They need the game (Steam) and Seamless Co-op v1.1.3 already installed. Nothing else is required: me3 is bundled.
Never use the Steam Play button with the mod, and never start `start_protected_game.exe` (CLAUDE.md hard rule 2).

## 1. What you need on the build PC (once)

| Thing | Where / version | Check |
|---|---|---|
| Game | ELDEN RING NIGHTREIGN 1.03.3, exe 1.3.3.0 | pins in [TECHNICAL_NOTES.md](TECHNICAL_NOTES.md) §1; if the game updated, **stop**: offsets must be re-verified |
| Seamless Co-op | v1.1.3 in `<Game>\SeamlessCoop\` | same pins |
| Rust | stable, `x86_64-pc-windows-msvc`, `%USERPROFILE%\.cargo\bin` | `cargo --version` |
| MSVC Build Tools | `D:\Programs\Microsoft Visual Studio` | "link: extra operand" means MSVC is missing |
| me3 | 0.11.0 in `Z:\Tools\me3` (`bin\me3.exe`, `me3-launcher.exe`, `me3_mod_host.dll`, `LICENSE-*`) | the packaging script checks these |
| Game unpack + WitchyBND + Python | `Z:\Tools\nr-unpacked\1.03.3\`, WitchyBND 3.0.1 in `Z:\Tools\` | only for `build-gamedata.ps1` |

## 2. Settings that decide what a release does

Players' settings live in `NightreignPvP\nrpvp.toml` inside the zip. `package-release.ps1` writes it from its parameters, so **do not hand-edit
the zip**: change the parameters and rebuild.

| Script parameter | What it sets | Typical value |
|---|---|---|
| `-MatchPlayers N` (2..6) | `mm_max_players` = `mm_min_players` = N: the queue lobby fills to exactly N players, then the match starts. **Everyone who should meet needs the same N** (lobbies with another N never match) | **2** for a test with one friend; **6** for the real game |
| `-TeamSize N` (1..3) | `team_size`: 1 = every player alone (Solo), 2 = pairs, 3 = trios | **1** (Duo / Trio queues are not built yet, MM-6) |
| `-Me3Dir DIR` | where the me3 binaries come from | default `Z:\Tools\me3` |

The generated file also contains `matchmaking = true` (the Battle Royale Solo queue in the table menu) and `[hawk] enabled = true`.

Settings a player or you may add to `nrpvp.toml` by hand after installing (all optional; defaults in `crates/nrpvp/src/config.rs`, which logs the
effective values at startup):

| Key | Default | Meaning |
|---|---|---|
| `day1_secs` | 1020 | real seconds of the free Day 1 + Night 1 (120..3600); 0 = vanilla Day 1 with tide and night. For a short test use 300 |
| `defeat_exit_secs` | 0 | seconds after a defeat until the defeat screen; 0 = watch until the match ends, F10 leaves |
| `spectate` | true | a defeated player's camera follows a living player |
| `private_code` | "" | players with the same code (letters, digits, `-`, `_`, up to 32) meet in a private queue, outside the public one |
| `mm_fill_wait_s` | 90 | the match starts this long after the 2nd member joined if at least `mm_min_players` are in |
| `matchmaking` | true | `false` turns the queue off entirely |
| `[hawk] enabled` | true | the start hawk crossing |

The player build ignores developer keys (`dev_mode`, the `mm_*` test switches); a developer DLL never goes into the zip.

## 3. Build the zip (the whole procedure)

From the repo root, in PowerShell, with the game **closed** (the build cannot replace a DLL in use):

```powershell
# 0. Is the code in a good state?  (all must pass)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p nrpvp --no-default-features --all-targets -- -D warnings
cargo fmt --all --check

# 1. Only if gamedata\*.json changed or dist\gamedata is missing (a fresh worktree has none):
powershell -ExecutionPolicy Bypass -File scripts\build-gamedata.ps1

# 2. (optional, recommended before a friend session) the automated scenarios; needs `nro desk` in a normal terminal and Steam running
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1

# 3. The zip. 2 players for a test with one friend:
powershell -ExecutionPolicy Bypass -File scripts\package-release.ps1 -MatchPlayers 2
#    For the real game later:   scripts\package-release.ps1 -MatchPlayers 6
```

`build-gamedata.ps1` writes `dist\gamedata`: one package with the Battle Royale, the Arena and saved loadouts (23 files). Every zip
takes it; there is no separate BR-only package or `-Arena` switch any more (DECISIONS 2026-11-13).

`package-release.ps1` builds the **player** DLL itself (`--no-default-features`, into `target\player\`) and the launcher, then packs. `scripts/build.ps1` is only for developer launches, not for the zip.

It prints `OK: ...\dist\NightreignPvP.zip (N MB)` and the two `VERSION.txt` lines, for example `version local` and
`built 2026-11-01 18:30 UTC, commit abc1234 (clean), nrpvp.dll 1A2B3C4D5E6F7788, gamedata emevd ..., hawk routes ..., me3 0.11.0`.
The first line is what the version gate (§5c) reads: `version X.Y.Z` when `-Version X.Y.Z` is given (`publish-release.ps1` and CI
always pass it), else `version local`, a test zip the gate never blocks.
**Build from a clean `main`** so the zip's commit is a real commit (the line says `dirty` otherwise).

### What is in the zip
`nrpvp_launcher.exe`, `HOW TO INSTALL.txt` (the player instructions, from `scripts\release\`), and `NightreignPvP\` with `nrpvp.dll`,
`nrpvp.toml`, `VERSION.txt`, `gamedata\` (15 languages + the Roundtable event script + the hawk line `map\mapstudio\m60_00_00_99.msb.dcx`;
the script refuses an incomplete set) and `me3\` (3 binaries + 2 licences). If you change player-visible rules, update
`scripts\release\HOW TO INSTALL.txt` in the same change (it still describes Solo queue only, which is right today).

## 4. Hand it over and test with one friend (2 players, Solo)

1. **Both PCs:** back up `%APPDATA%\Nightreign\<steamid>\` (never touch the vanilla `NR0000.sl2`). Same game exe 1.3.3.0 and Seamless Co-op v1.1.3;
   stop Steam auto-updates for the session.
2. Send the **same** zip to your friend (a link or file transfer). Each player unzips it and drags `nrpvp_launcher.exe` and the `NightreignPvP`
   folder into the game folder (next to `nightreign.exe` and `nrsc_launcher.exe`); "Replace the files" for an older version.
3. **Compare `NightreignPvP\VERSION.txt` on both PCs**: the `nrpvp.dll` hash must be identical. If not, someone has an old zip.
4. Steam running, start with `nrpvp_launcher.exe` (never the Steam Play button).
5. Both: Roundtable > table > the **Battle Royale** card > Matchmaking Settings > Number of Players = **Solo** > Commence. "searching for players (1 of 2)"
   appears; nobody presses anything after that: the match is found, both are brought together and the expedition starts by itself.
6. Play. For a quick check set `day1_secs = 300` at the top of both `nrpvp.toml` files before starting. The runbook with what to try and what
   to expect (kill, spectate, drops, F10, quit) is [TESTING.md](TESTING.md) §6.
7. After the session, collect from **both** PCs: `NightreignPvP\nrpvp.log`, the `NightreignPvP\logs\` folder, the me3 log folder
   (`%LOCALAPPDATA%\garyttierney\me3\data\logs\`), photos. Put them under `.claude\experiments\<session>\host\` and `...\guest\` (gitignored) and use the
   `test-report` skill to turn them into a `testing-log/` entry.
8. Record the build in [STATUS.md](STATUS.md) §2 (date, `nrpvp.dll` hash, flags, "not installed by an agent") and say which older zips must not be sent.

If the game crashes on quit (0xC0000374) it is the known bug; note the time and send the logs anyway.

## 5. Later: more players (autonomy checklist)

- **Same N everywhere.** Build with `-MatchPlayers 6` and give every player that exact zip. A player on another N or an older build never sees the
  others (lobby filters `need`, `proto`, `game`, `sc`). If a match does not form, compare the `need=` and `proto` lines in the logs (MATCHMAKING.md, lobby data).
- **6 players is verified only in automated tests**; the real queue has been run with 2. Expect to find problems and keep the logs.
- **Duo / Trio** queues do not exist yet (the menu says "coming soon"); `team_size = 2/3` is only for tests (TASKS DUOTRIO-1, MM-6).
- **Private groups:** give every player the same `private_code` line to keep strangers out.
- **After a game or Seamless Co-op update:** re-hash, update the pins (`TECHNICAL_NOTES.md` §1, `pins/nightreign.toml`), re-run `regress.ps1`
  and the `mi-*` scenarios, then rebuild.
- **Rollback:** keep the previous zip (name it by date); a player restores by dropping that zip's files over the game folder. Uninstall = delete
  `nrpvp_launcher.exe` and the `NightreignPvP` folder.

## 5b. Publish for the launcher app (LAUNCH-1)

The launcher app (`crates/nrpvp-app`, `NightreignPvP.exe`) installs and updates the mod from GitHub releases of **this public
repository**, `notgonnaleo/nightreign-pvp` (since 2026-11-23, DECISIONS). Each release holds `release.json`,
`NightreignPvP.zip` and `NightreignPvP-Launcher.exe` (plus GitHub's source archives, harmless now that the source is public).

**Download page** in `docs/` here (moved from the legacy repository on 2026-11-23): static `index.html` + `site.css` on Bootstrap
5, theme tokens in `design/tokens.json` → `node design/build-tokens.mjs` → `docs/ui/tokens.css`; GitHub Pages from `main` `/docs`,
https://notgonnaleo.github.io/nightreign-pvp/ (no game art; the copy sells Battle Royale and Arena as PvP). Its buttons link
`releases/latest/download/NightreignPvP-Launcher.exe`, so a new release needs no page edit; the version line and "What's new" come
from the GitHub API in the browser (with no release yet they fall back to "Free · Windows"). Pushes that only touch `docs/` or
`design/` never build a release (`paths-ignore`). Preview locally: `.claude/launch.json` config `download-page`.

**Old releases repository** `aspherya/nightreign-pvp-releases` (2026-11-13 to 2026-11-23, up to v0.3.3) **was deleted on
2026-11-23** [VERIFIED: API 404 with an org member's token, org lists 0 repositories], before v0.4.0 could be copied there. Launchers
up to v0.3.3 have it built in: their update check fails, so the version gate stops them ("Could not check for updates"). Those
players must download the new launcher from the download page once. Launchers v0.1.3 and v0.1.7 read `aspherya/nightreign-pvp`, which GitHub now redirects here [HYPOTHESIS: they follow the
redirect and see the new releases]. One command
builds the zip, the launcher and `release.json`, and uploads them:

```powershell
./scripts/publish-release.ps1 -Version 0.2.0 -NotesFile notes.txt -MatchPlayers 2 -DryRun   # stage in dist\release\, upload nothing
$env:NRPVP_RELEASE_DIR = "$PWD\dist\release"; .\dist\release\NightreignPvP-Launcher.exe     # try the staged release locally
./scripts/publish-release.ps1 -Version 0.2.0 -NotesFile notes.txt -MatchPlayers 2           # publish (gh auth login once)
```

- **Release policy (2026-11-13):** pull requests are reviewed before merging. A successful push to `main` publishes the new update automatically; no additional draft approval is required. Manual workflow runs can explicitly request a draft.
- The newest non-draft release is what every launcher installs: publishing is the release. Never reuse a version number.
- `release.json` pins the game exe and `nrsc.dll` hashes from `pins/nightreign.toml`: the launcher warns a player whose game or Seamless
  Co-op differs. Update the pins when the game or SC changes.
- Players download `NightreignPvP-Launcher.exe` once. On startup, with the game closed, the launcher installs newer published launcher and mod versions. A manually installed mod without version metadata adopts the latest published release once. Downloads are checked against `release.json`; the launcher update helper waits for the old process to exit, replaces it beside a backup, and restarts. Version comparison prevents downgrades of versioned installations and repeat updates. `--skip-auto-update` disables automatic updates for troubleshooting.
- Testing without GitHub: `NRPVP_RELEASE_DIR=<folder with release.json + assets>`; another repository: `NRPVP_RELEASES_REPO=owner/name`.
- **Private room** (RW-6): the launcher's Private room tab writes `private_code` into the installed `nrpvp.toml` (before the first table); an update keeps it. Friends save the same code, then queue as usual. Mock: `?room=night-owls`.
- The launcher warns (Install tab and the bottom bar, red) when files of another tool sit in the game folder and make the game load other code: `winmm.dll`, `dlllist.txt`, `OnlineFix*`, `steam_emu.ini`, `ColdClientLoader.ini`, `steam_settings` (an OnlineFix package broke every run on 2026-11-08, TRAPS). It removes nothing. Mock: `?foreign`.
- UI look without building: `.claude/launch.json` config `launcher-ui-mock` serves `crates/nrpvp-app/ui` on a mock (`?fresh`, `?update`).

## 5c. Version gate: an old mod does not start (LAUNCH-GATE)

Everyone in a match needs the same version, so **`nrpvp_launcher.exe` and the launcher app's Play refuse to start the game when the
installed mod is older than the latest published release** (both run the same code, `nrpvp_launch::launch` → `version::gate` in
`crates/nrpvp-launcher`, which uses the same release lookup as the app's updater).

- **Installed version:** the `version X.Y.Z` line of `NightreignPvP\VERSION.txt`; if that file has none, the version the app recorded
  in `NightreignPvP\release.json`. **Latest:** the newest stable release's `release.json` (GitHub, no token; `NRPVP_RELEASE_DIR` /
  `NRPVP_RELEASES_REPO` work here too).
- **Older, or no version at all** (a zip from before the gate): blocked. `nrpvp_launcher.exe` shows a box: "Your Nightreign PvP is
  vA, but vB is out ... open the launcher app (it updates by itself) or download the new NightreignPvP.zip from
  `https://github.com/<releases repo>/releases/latest` and replace the files", with **Yes** = open that page. The app
  shows **Update required: vB** in red at the bottom with the Update button (the old "Play without updating" link is gone); if Play
  is pressed with a stale page, it shows the same message and turns to Update.
- **Same or newer:** starts. **GitHub not reachable** (offline, the 60-per-hour limit, an error; 10 s limit per request): **does
  not start** (DECISIONS 2026-11-17, Leonardo). The exe says "could not check for the latest version ... connect to the internet and
  try again" with the reason; the app shows "Could not check for updates" with **Retry** instead of Play. A GitHub outage therefore
  stops every player until it passes. **`version local`** (a zip built without `-Version`): starts without asking GitHub.
  **`NRPVP_SKIP_VERSION_CHECK=1`**: starts (developer escape).
- Every start writes one `version gate: ...` line at the top of `NightreignPvP\launcher.log` (OK, or BLOCKED with the versions or
  "latest unknown (why)"). UI mock: `?update` (old version), `?offline` (no check).
- Developer launches (`scripts\launch.ps1`, `nro`) never go through `nrpvp_launch`, so the gate never touches them.
- Launchers and zips from before the gate do not have it: a player is first gated once their zip or app has it.

## 6. Quick troubleshooting

### GitHub Actions release setup

`.github/workflows/release.yml` runs on every push to **main** that changes more than docs, or **Actions > Build player release >
Run workflow**. Pushes that only touch `.claude/**`, `*.md` or `scenarios/**` do not run: a docs change needs no release. There is
no CI on pull requests (removed 2026-11-13 to save minutes while the repository was private; standard runners are free on a public
one, so it could come back); run `cargo test` / `cargo clippy` locally before merging.

One job, in order: tests and clippy in the **debug** profile (the release profile's thin LTO with one codegen unit is slow and only
needed for shipped binaries); a release build of the launcher, helper and player DLL (no developer features) in one shared `target`
folder (`NRPVP_PLAYER_TARGET_DIR`), so dependencies compile once; the pinned build inputs; `publish-release.ps1 -DryRun` packages
`NightreignPvP.zip`, `NightreignPvP-Launcher.exe` and the SHA-256 `release.json`; `publish-ci-release.ps1` uploads them to a
temporary **draft** in this repository (built-in `GITHUB_TOKEN`) and publishes it as the latest stable release only after every
upload succeeds. `Swatinem/rust-cache` keeps
compiled dependencies between runs. No workflow artifact is uploaded: the release holds the files. A manual run can keep the release
a **draft** (players never see drafts). Never reuse a version or replace already-published assets. Update `releases/notes.txt` before merging.

Runs with no explicit version take the higher of the workspace version (`Cargo.toml`) and one patch above the highest version ever
used in this repository, drafts included, so versions always increase by one and a test draft's number is never reused. A minor
or major step is a workspace version bump. (Until 2026-11-13 the run number was added to the patch, and the new repository restarted at
0.1.x below the 0.2.0 that earlier testers had installed; the launcher refuses downgrades, so the workspace version went to 0.3.0;
the move back to this repository on 2026-11-23 went to 0.4.0.) An explicit manual version is used as given. A rerun can replace unpublished draft assets only when that
draft was built from the exact same commit (its body names the source commit); published releases cannot be overwritten.
Tags point at the source commit.

One-time setup by the repository owner:

1. Keep this repository **public**: player releases go here (`github.repository`; override with the Actions variable
   `RELEASES_REPO`), the workflow embeds that name in the launcher, which reads it without a token, and publishing refuses to run
   while the target is not publicly readable. Build inputs stay a **draft** here (never visible to the public). The built-in
   `GITHUB_TOKEN` (workflow `contents: write`) publishes: no personal token or Actions secret is needed (the old
   `RELEASES_PUBLISH_TOKEN` is unused since 2026-11-23).
2. CI builds the game data itself on every release (`build-gamedata.ps1`, step **Build game data**), from pinned inputs: the
   original game 1.03.3 files it reads, WitchyBND, ESDLang 0.5.1, the Smithbox 2.2.6 DLLs and me3. Nothing is compiled to make them.
   On the machine holding the pinned game unpack and tools:

   ```powershell
   ./scripts/prepare-ci-inputs.ps1 -Unpacked Z:\Tools
r-unpacked.03.3 -Witchy Z:\Tools\WitchyBND-3.0.1.0\WitchyBND.exe -EsdTool Z:\Tools\ESDLang-0.5.1\esdtool-v0.5.1\esdtool.exe -Smithbox Z:\Tools\Smithbox-2.2.6 -Me3Dir Z:\Tools\me3
   ```

   Redo it only after a game patch or a tool update. Upload `dist/ci-inputs.zip` to a **draft** release in this repository (it holds
   game files: never publish it). Keep it a draft so it
   never becomes a player update.
   `pins/ci-inputs.json` records the uploaded draft's release ID and SHA-256 (the only source CI uses).
   Unpublished draft tags do not resolve through the tag API, so CI downloads the asset by its pinned release ID.
   When regenerating inputs, upload a new draft and update that file with its ID and the new hash. Find a draft ID with
   `gh api repos/notgonnaleo/nightreign-pvp/releases --jq '.[] | select(.tag_name == "build-inputs-v1") | .id'`.
   This bundle contains only the generated player
   game-data package and pinned me3 binaries/licenses, never Seamless Co-op or a game executable.
   Regenerate and update the hash when game-data sources/pins change. A hosted Windows runner cannot generate
   game content without the local unpack and tools. The currently installed Arena test package is not a player input.
   The current player input has 21 files, including `menu/05_000_title.gfx`. Regenerate older bundles to include the custom title background.
3. The workflow's built-in `GITHUB_TOKEN` downloads the input draft and creates and publishes the player release here.
4. Push the reviewed change to `main`. The workflow runs launcher/helper tests and player-DLL tests/lint, builds the complete
   package and publishes it. Failures leave the previous stable release active. To test without publishing, use a manual workflow
   run with **draft**, or build locally with `NRPVP_RELEASE_DIR`.

### How the launcher reads releases

Without any token: the releases repository is public. The launcher never sends one, and none is built into the exe or committed.
Launchers up to v0.1.x had a **Settings > Private updates** token field; a token saved there made GitHub answer 401 ("invalid or
expired") even for the public repository, so the current launcher removed that field and deletes
`%LOCALAPPDATA%\NightreignPvP\github-token.dpapi` on startup (TRAPS). Without a token GitHub allows 60 API requests per hour per
IP; an update check uses a few.

The launcher uses the release API to locate `release.json`, then the asset API with `Accept: application/octet-stream`
to download the manifest, launcher and player zip, building every URL from `api.github.com/repos/<owner>/<repo>/...` and the
numeric asset ID (never from a URL in the manifest). SHA-256 verification still applies. Drafts and prereleases are excluded from
player updates. `NRPVP_RELEASE_DIR` still supports offline release testing.

### Player save setup and name changes

The launcher embeds `crates/nrpvp-app/assets/starter.sl2`, extracted from the supplied RAR (provenance and hashes in
that directory's README). No separate save generation or player download is required. First setup (or Play when no
Seamless save exists) creates the configured Seamless save for the selected SteamID64 and rebinds its embedded owner.
The active Steam account is read from Steam's live process/registry at startup, on focus and for every Play/setup/edit operation.
There is no manual SteamID entry or saved-account fallback; account switches are respected automatically. If Steam is closed or
signed out, the launcher asks the user to start Steam and sign in. Destructive replacement and name edits reject an account switch
between the displayed confirmation and the operation.
Existing saves are preserved. The vanilla `.sl2` extension is refused. Setup and name edits require the game closed.
Settings lists supported nonempty character slots; **Save name** edits that slot's UTF-16 player name and recomputes
the encrypted entry checksum. Backups named `NR0000.<extension>.nrpvp-<timestamp>.bak` sit beside the save.
Past match-history names are preserved. A supported-format roundtrip does not prove in-game compatibility: test
starter-save loading and the changed name in the pinned game before publishing.

**Settings > Player > Use all-unlocked save** is available after initial setup too. It requires an explicit confirmation,
backs up the existing Seamless save (even a damaged one), rebinds the bundled template to the live Steam account and replaces
the save atomically. A readable character name from the same account is kept. The game must be closed; vanilla saves are untouched.

**Settings > Player > Import Steam save** copies the signed-in account's local vanilla
`%APPDATA%\Nightreign\<SteamID64>\NR0000.sl2` into the configured Seamless Co-op save. It verifies every entry checksum and
the embedded owner before replacing anything. The entire file is copied byte-for-byte, preserving all original character
names, profile data and progress; the source `.sl2` is never written. An existing mod save is backed up first, and a new
mod save can also be created this way. Settings previews the detected character names. Both save sources use an explicit
replacement confirmation. Close the game first; if the local vanilla save is missing, start the normal game once and let
Steam synchronize it. This imports the local Steam save, not a direct Steam Cloud download.

| Symptom | Cause / fix |
|---|---|
| `dist\gamedata incomplete` / "map must hold exactly ..." | run `scripts\build-gamedata.ps1` (without `-VanillaRoutes`) |
| `me3 file missing` | `-Me3Dir` wrong or me3 not 0.11.0 in `Z:\Tools\me3` |
| cargo build of the launcher / player DLL failed | MSVC missing, or a game still running from `dist\` / `target\player\` |
| Launcher says "must be in the game folder" | files dropped in the wrong folder; the game folder holds `nightreign.exe` |
| Launcher says Seamless Co-op was not found | player must install SC v1.1.3 first |
| Players never meet | different `-MatchPlayers`, different mod build, different SC version, or different `private_code` |
| The zip's commit says `dirty` | build from a clean `main` for a session you want to be reproducible |
| "Your Nightreign PvP is vA, but vB is out" | the version gate (§5c): update through the launcher app or the new zip; a friend-test zip must be built without `-Version` (`version local`) |
| "could not check for the latest version" / "Could not check for updates" | no internet, GitHub down, or GitHub's 60-per-hour limit for that IP (§5c, "How the launcher reads releases"): connect, or wait and retry |
