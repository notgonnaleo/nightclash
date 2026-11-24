<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/flame-dark.svg">
    <img src="docs/brand/flame-light.svg" alt="Flame icon" width="140">
  </picture>
</p>

# NIGHTREIGN PVP

A PvP mod for **ELDEN RING NIGHTREIGN** with two modes: **Battle Royale**, where players land on Limveld as rivals, fight each other
and the map's enemies, and the last player or team standing wins; and **Arena**, a best-of-three duel in Heolstor's arena. Players
queue from the game's own expedition menu.

**Download:** https://notgonnaleo.github.io/nightreign-pvp/ (the launcher), or the
[latest release](https://github.com/notgonnaleo/nightreign-pvp/releases/latest).

It runs on top of [Seamless Co-op](#requirements), with no anti-cheat and no connection to the official matchmaking servers. There is no
server of ours and no overlay: players find each other through Steam lobbies, and everything on screen is the game's own menus and messages.

> **Status: pre-release.** Battle Royale Solo and Arena 1v1 are playable and have been played over real Steam. The current state of
> every feature is in [STATUS.md](.claude/STATUS.md).

**Contents:** [For players](#for-players) · [The rules](#the-rules) · [What works today](#what-works-today) ·
[For developers](#for-developers) · [The test orchestrator (`nro`)](#the-test-orchestrator-nro) · [Project layout](#project-layout) ·
[Documentation](#documentation) · [Safety and legal](#safety-and-legal)

---

## For players

### Requirements

- Windows, and ELDEN RING NIGHTREIGN on Steam, version **1.03.3**
- Seamless Co-op for Nightreign, version **1.1.3** (the launcher helps you install it)
- An internet connection: every start checks that you have the latest version, because everyone in a match needs the same one

The mod brings its own copy of [me3](https://github.com/garyttierney/me3), the mod loader.

### Install with the launcher (recommended)

1. Back up `%APPDATA%\Nightreign`, where the saves live.
2. Download **`NightreignPvP-Launcher.exe`** from the [download page](https://notgonnaleo.github.io/nightreign-pvp/) and run it.
3. Follow its first-run steps: it finds the game folder, opens Seamless Co-op's Nexus page if it is missing and installs the zip you
   download, then installs the mod.

The launcher keeps itself and the mod up to date: on each start, with the game closed, it installs a newer release by itself.

### Install by hand

1. Back up `%APPDATA%\Nightreign`.
2. Download `NightreignPvP.zip` from the [latest release](https://github.com/notgonnaleo/nightreign-pvp/releases/latest).
3. In Steam, right-click the game > **Manage** > **Browse local files**, then open `Game`. It is the folder with `nightreign.exe` and
   `nrsc_launcher.exe`.
4. If an older version is there, delete `nrpvp_launcher.exe` and the `NightreignPvP` folder.
5. Drag `nrpvp_launcher.exe` and the whole `NightreignPvP` folder from the zip into that folder.

**To uninstall,** delete the same two items (and the launcher app, if you used it). Nothing else is changed.

### Play

1. Start Steam, then press **Play** in the launcher (or double-click **`nrpvp_launcher.exe`** in the game folder).
2. At the Roundtable table, open the expedition list and pick the **Battle Royale** or the **Arena** card, at the end of the list.
3. In **Matchmaking Settings**, set **Number of Players** to **Solo**. Set **Bots** to **On** to practise offline against AI Nightfarers.
4. Press **Commence**. To stop searching, press the same button again.

That is the last click. When the match is found, everyone is brought together and the match starts by itself, about a minute later.

> The mod only loads through `nrpvp_launcher.exe`. Steam's **Play** button starts the normal game with anti-cheat and without the mod, which
> is how you play the normal game. Any other card in the list is also a normal expedition.

### In a match

| Situation | What happens |
|---|---|
| Start | Everyone rides the hawk along one line across the map and chooses where to jump |
| You are killed by an enemy, a boss or a fall | Normal death: you respawn, far from other players |
| You are at 0 HP after a rival's hit | You are knocked. Each further hit from a rival takes 3 s off your 15 s bleed-out |
| You are defeated by a rival or the Night's Tide | You are out for good, and your camera follows a living player |
| While you watch | Mouse buttons or d-pad left / right switch player, **F10** leaves to the Roundtable, **Esc** or pad **Start** open the menu |

### Settings

Optional. Add them to `NightreignPvP\nrpvp.toml` (the launcher's **Private room** tab sets `private_code` for you). Match settings must be the same on every PC.

| Key | Default | Meaning |
|---|---|---|
| `day1_secs` | `1020` | Real seconds of the free Day 1 and Night 1 (120 to 3600). `0` is the vanilla Day 1 |
| `defeat_exit_secs` | `0` | Seconds from a defeat to the defeat screen. `0` means watch until the match ends |
| `spectate` | `true` | A defeated player's camera follows a living player |
| `private_code` | empty | Players with the same code meet in a private queue |
| `matchmaking` | `true` | `false` turns the queue off |

### If something goes wrong

- **"Update required" or "Your Nightreign PvP is vA, but vB is out":** let the launcher update, or download the new zip.
- **"Could not check for updates" / "could not check for the latest version":** the mod needs the internet to start. If you have it and
  your launcher is v0.3.3 or older, that launcher looks in a place that no longer exists: download the new one from the
  [download page](https://notgonnaleo.github.io/nightreign-pvp/).
- **"This launcher must be in the game folder":** the files are in the wrong place. Go back to [Install by hand](#install-by-hand), step 3.
- **"Seamless Co-op was not found":** install Seamless Co-op first.
- **The game does not start:** make sure Steam is running.
- **Windows asks to run the file:** choose **Run**, or **More info** > **Run anyway** on a blue "Windows protected your PC" box.
- **Never matched:** everyone needs the same version. Compare `NightreignPvP\VERSION.txt` on each PC.

To report a problem, send the `NightreignPvP\logs` folder.

---

## The rules

### Battle Royale

Battle Royale is built for 6 players: 6 solos, 3 duos or 2 trios. Pre-release zips usually start at 2 players, so a test needs only one friend.

- **PvP is always on.** Rivals are marked as hostile automatically, and normal enemies keep fighting everyone.
- **Day 1 and Night 1 are free:** no Night's Tide, no night and no boss. Day 2 and Night 2 use the vanilla tide, which shrinks the map.
- **No Day 3 and no Nightlord.** The match ends the moment one team is left, on any day.
- **Rivals are hidden:** no map or compass mark, party bar or name tag, and their map pins are hidden too. A locked-on target keeps its name tag.
- **Defeat drops:** a rival's kill drops the victim's weapons (level kept), talismans and picked-up consumables where they fell, for anyone to
  take. Runes are discarded.
- **Runes from enemies** go only to the killer's team.
- **Enemy strength** follows the team size, not the number of players in the session.
- **Sites of Grace** give nothing to a player in a PvP fight, until 15 s after the last hit or until no rival is near.
- **Killfeed:** `A downed B` and `A killed B` appear as the game's own messages, with character names.

### Arena

A 1v1 duel in Heolstor's arena (no boss, no expedition). Duo and Trio are planned.

- **Best of three:** the first to win two rounds wins the match.
- **Even ground:** everyone is level 15, every weapon at its maximum upgrade.
- **Your build:** you fight with the loadout you made with the Sparring Equipment in the Sparring Grounds; without one, the Nightfarer's
  default weapon.
- **A round ends at 1 HP:** nobody dies or is downed; the first player brought to 1 HP loses the round.
- **3-minute rounds:** "One minute left!" comes 60 s before the limit. Then up to 60 s of overtime drains both players' HP; the first at
  1 HP loses. If nobody reaches it, the higher HP in percent wins.
- **Every round** starts with full HP, FP and flasks, your consumables back, and a full Ultimate Art gauge that does not charge again in
  that round.

The full specification is in [PROJECT_SCOPE.md](.claude/re/PROJECT_SCOPE.md).

---

## What works today

What works, what is proven where (real Steam or one PC), and what is still open lives in one place only:
**[STATUS.md](.claude/STATUS.md)**. This README does not copy it, so it never goes stale here.

---

## For developers

### Build requirements

| Tool | Version | Note |
|---|---|---|
| Rust | stable, `x86_64-pc-windows-msvc` | Pinned in `rust-toolchain.toml` |
| MSVC Build Tools | | "link: extra operand" means they are missing |
| me3 | 0.11.0 | Expected in `Z:\Tools\me3` |
| Game + Seamless Co-op | 1.03.3 + 1.1.3 | Hashes in `pins/nightreign.toml`, which also holds the game path |
| Game unpack + Python | | Only for the game data, see [TOOLS.md](.claude/TOOLS.md) |

The mod is tied to one game version. **If the game updates, stop:** the addresses must be verified again.

### Build and run

Windows users can double-click **`build-launcher.bat`**, **`build-mod.bat`** or **`build-all.bat`** in the repository root.
The portable batch files detect Rust/MSVC and put binaries in `dist/`. Game-data, release and CI-input batch files are also
provided; see [BUILDING.md](BUILDING.md) for prerequisites, local tool paths and command-line options.

Run from the repository root, with the game closed. The build cannot replace a DLL in use.

```powershell
powershell -NoProfile -File scripts\build.ps1                     # developer DLL, nrcon and nro
powershell -ExecutionPolicy Bypass -File scripts\build-gamedata.ps1  # event scripts, texts, regulation.bin
powershell -ExecutionPolicy Bypass -File scripts\launch.ps1       # start the game with the developer build
```

While a developer game runs, drive it with the console (commands in [CONSOLE.md](.claude/CONSOLE.md)):

```powershell
dist\nrcon.exe status
dist\nrcon.exe players
dist\nrcon.exe log 30
```

### Build a player zip

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package-release.ps1 -MatchPlayers 2
```

This writes `dist\NightreignPvP.zip` and builds its own player DLL, without the developer tooling. `-MatchPlayers N` (2 to 6) is how many
players start a match. Everyone who should meet needs the same N. The full procedure is in [RELEASE.md](.claude/RELEASE.md).

### Releases

Work goes to `dev` through pull requests; `dev` is merged into `main` for a release. **Every push to `main` that changes shipped code
publishes a new release of this repository by itself** (`.github/workflows/release.yml`: tests, game data from pinned inputs, the zip,
the launcher and `release.json`), and every launcher picks it up. Pushes that only touch docs, `scenarios/`, `docs/` or `design/`
do not. Write the player-facing notes in `releases/notes.txt` before merging. Details: [RELEASE.md](.claude/RELEASE.md) §5b and §6.

### Checks before a commit

There is no CI on pull requests: run these before merging. The release workflow runs the launcher and player-DLL subset again on `main`.

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p nrpvp --no-default-features --all-targets -- -D warnings
cargo test --workspace
```

---

## The test orchestrator (`nro`)

`nro` launches a **developer** copy of the game through me3, never online and without anti-cheat. It plays it like a player (console, keys,
screenshots), can fill it with AFK puppet players, and runs **scenarios**: lists of steps with checks. Every command prints JSON, and everything
it writes goes to `runs\<id>\`.

### Setup

1. Build as above, including the game data.
2. Start the desk in a **normal terminal** and leave it open. It serves keys and screenshots:

   ```powershell
   New-Item -ItemType Directory -Force runs | Out-Null
   Copy-Item target\release\nro.exe runs\nro-desk.exe
   runs\nro-desk.exe desk
   ```

   Running a copy keeps `target\release\nro.exe` free for the next build.
3. In your working terminal, make the command short:

   ```powershell
   Set-Alias nro .\target\release\nro.exe
   ```

Keep the PC awake, unlocked and untouched while a run uses keys. `nro env` shows the versions, the relevant processes and the current session.

### Run one game by hand

Steam must be running.

```powershell
nro launch --world --set mode=offline   # safety checks, launch, press through to the Roundtable
nro navigate expedition                 # Roundtable to the offline Solo expedition
nro puppets 3                           # 3 AFK puppet players, who count as rivals
nro cmd players                         # any console command
nro cmd "night_status"                  # what the match rules see
nro keys "F; wait 2s; click 0.5 0.5"    # keys and mouse into the game window
nro shot after_kill                     # screenshot into runs\<id>\shots\
nro log 50 night_match                  # last 50 mod log lines containing that text
nro stop                                # always end with this
```

### Run scenarios

```powershell
nro scenarios --tag smoke                                                # list them; tags: smoke rules hud mi slow lab
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1 -Tag smoke  # the quick sanity pair
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1 day1-free   # by name, one PASS / FAIL line each
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1             # everything except the mi and lab tags
nro run scenarios\day1-free.toml                                         # one scenario with the full JSON report
```

**PASS** means every check held. **FAIL** means the game did something else than expected, so read the log around that step. **ABORTED** means
it never got there, for example a menu did not open or the PC locked. The results are in `runs\<id>\report.json`, `mod\nrpvp.log` and `shots\`.

### Two real instances on one PC

For anything that needs two games: matchmaking, drops seen by both, hidden rivals. Each instance gets an emulated Steam id, so **Steam must be
closed**.

```powershell
nro stop --all
nro run scenarios\mi-zero-click.toml   # the whole player flow, no click after Commence
```

To explore by hand, add `--i N` to any command:

```powershell
nro launch --emulated 1
nro launch --emulated 2
nro cmd players --i 2
nro shot a --i 1
nro stop --all
```

This needs about 20 GB of free RAM for two games, the emulator files from the Nucleus handler folder (or the folder in `NRO_EMU_SRC`), and
Allow-Private firewall rules for the instance folders. Run `nro` from a normal terminal or PowerShell. Only one run at a time works on a PC.

### Write a scenario

Copy the nearest file in `scenarios\`. `[config]` keys go over the staged `nrpvp.toml`, then the steps run in order:

```toml
name = "my-check"
about = "One line: what this proves"

[config]
mode = "offline"
mm_queue = false
ui_flag_watch = false

[[step]]
navigate = "title_to_world"
[[step]]
navigate = "expedition"
[[step]]
puppets = 1
[[step]]
console = "rival_hit"
[[step]]
console = "hp0"
[[step]]
wait = { console = "night_status", contains = ["defeated by a rival"], timeout_s = 30 }
[[step]]
screenshot = "done"
```

Reach a state with console commands rather than keys. Every step type is in [GUIDE.md](.claude/orchestrator/GUIDE.md), and what each scenario proves is
in [SCENARIOS.md](.claude/orchestrator/SCENARIOS.md).

---

## Project layout

| Path | What it is |
|---|---|
| `crates/nrpvp` | The mod (`nrpvp.dll`) |
| `crates/nrpvp-launcher` | `nrpvp_launcher.exe`, the player's one-click start, and the release lookup and version check |
| `crates/nrpvp-app` | `NightreignPvP-Launcher.exe`, the launcher app (Tauri): install, updates, Play, private room |
| `crates/nrcon` | `nrcon.exe`, the console client for a running developer game |
| `crates/nro`, `nro-core`, `nro-mcp` | The orchestrator and its MCP server |
| `gamedata/` | Sources of the game data: texts, event scripts, the Battle Royale card |
| `scenarios/` | Automated test scenarios |
| `profiles/`, `pins/` | me3 profiles, and the versions and hashes every launch is checked against |
| `scripts/`, `tools/` | Build, launch, packaging and regression scripts, and the regulation writer |
| `docs/`, `design/` | The download page (GitHub Pages from `main` `/docs`) and its design tokens |
| `releases/notes.txt`, `.github/` | Notes for the next release, and the release workflow |
| `.claude/` | All project documentation |

Inside the mod, game memory lives in `crates/nrpvp/src/game/`, matchmaking in `mm/`, the Steam calls in `steam/`, the match rules in `features/` and
the data channel between games in `net/`.

## Documentation

| Read | For |
|---|---|
| [STATUS.md](.claude/STATUS.md) | What works, what is deployed, what is next |
| [PROJECT_SCOPE.md](.claude/re/PROJECT_SCOPE.md) | The product specification |
| [ARCHITECTURE.md](.claude/ARCHITECTURE.md) | Code structure and coding rules |
| [MATCHMAKING.md](.claude/MATCHMAKING.md) | How the queue and the match start work, and how to read their logs |
| [MEMORY_MAP.md](.claude/re/MEMORY_MAP.md) | Every game address we use and how it was confirmed |
| [TOOLS.md](.claude/TOOLS.md), [CONSOLE.md](.claude/CONSOLE.md) | Build tools, test routes, console commands |
| [RELEASE.md](.claude/RELEASE.md) | Building and releasing the player zip and the launcher |
| [TESTING.md](.claude/TESTING.md), [TRAPS.md](.claude/TRAPS.md) | Test plans and runbooks, and mistakes already made |

[.claude/CLAUDE.md](.claude/CLAUDE.md) has the full map of the documentation and the working rules of the project.

## Safety and legal

- **Never go online with a modded game.** Start it only through the launcher, `nrpvp_launcher.exe`, `scripts\launch.ps1` or `nro`, which run without
  EasyAntiCheat. Never start `start_protected_game.exe` with mods present.
- Back up `%APPDATA%\Nightreign` before you install. Mod sessions use Seamless Co-op's own save files, separate from the vanilla save.
- Seamless Co-op is closed-source third-party software. This project does not modify or redistribute it: players install it themselves, and the mod only
  works alongside it at run time.
- This is an unofficial, free, non-commercial fan project, not affiliated with or endorsed by FromSoftware, Bandai Namco or the author of
  Seamless Co-op. ELDEN RING NIGHTREIGN is a trademark of its owners, and you need your own copy of the game on Steam. Use the mod at your own risk.

## License

The code in this repository is licensed under the **GNU General Public License v3.0 or later** ([LICENSE](LICENSE)). You may use, change and
share it; any version you share, changed or not, must stay open source under the same license.

The license covers only this project's own work. It does not cover content that belongs to others:

- **The game's content** belongs to FromSoftware and Bandai Namco: its art, and the data in `gamedata/` and in the game-data package that is
  taken from or based on the game's files. The flame icon at the top of this README is based on an icon from the game.
- **me3**, bundled in the player zip, keeps its own license (MIT / Apache-2.0); its license files ship with it.
- **Seamless Co-op** is not part of this repository or the player zip.
