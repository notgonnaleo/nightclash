# Status (updated 2026-11-23)

**The only "now" doc.** Rules: keep it under ~8 KB; replace lines instead of appending history (results go to `testing-log/`, traps to
[TRAPS.md](TRAPS.md), decisions to [DECISIONS.md](DECISIONS.md), tools to [TOOLS.md](TOOLS.md), live cards to [TASKS.md](TASKS.md)).

## 1. What the mod does today

Two modes ship together in every release: **Battle Royale** (`MODE-01`) and **Arena** (`MODE-02`), rules in
[re/PROJECT_SCOPE.md](re/PROJECT_SCOPE.md). "One PC" = two emulated instances under `nro`; only real Steam proves networking.

| Feature | State |
|---|---|
| PvP and PvE together, rivals marked hostile, teams from player IDs | **Works on real Steam** |
| Solo matchmaking, no click after Commence (the host's game makes the lobby, everyone ready, the expedition starts by itself) | **Finished on real Steam** (2026-11-07, 3 players, as host and joiner, no Steam window or notification). 6 players only in automated tests |
| Battle Royale and Arena cards in the Roundtable expedition list | **Works**; any other card is a vanilla expedition |
| Match rules: rival or tide kill = out for good, mob / boss / fall = the game's respawn, free Day 1, the match ends when one team is left, the final circle closes to a point (TIDE-MAX) | **Works**; tide defeat and TIDE-MAX PASS on one PC |
| Spectating after a defeat (switch player, F10 leaves, hint) | **Works on real Steam** (2026-11-07). Camera smoothness open |
| Defeat drops (weapons, talismans, consumables; runes discarded), team-only runes, hidden rivals (map, compass, name tags, pins) | **PASS on one PC**; Duo / Trio teammates open. RW3-BIGINV: drop skips talismans on a save with many relics |
| Killfeed, Sites of Grace off in a PvP fight (GRACE-1) | **Works on real Steam** (2026-11-07) |
| K/D scoreboard (own score in the pause menu, everyone's after the match; Arena round score) | **Works on real Steam**; its bugs fixed in FS7-1 / FS7-7, real-Steam check open |
| Host migration (the game's own, also after the host's Alt-F4) | **Works on real Steam** (2026-11-07) |
| Hawk crossing at the start | **Works solo**; not seen in a friend session |
| Duo / Trio, MM-FILL (fill a short party with random players) | **Built**: a Duo party and a filled Duo start a match on one PC. 3 teams, Trio and real Steam open (DUOTRIO-1, needs 4+ players) |
| Private room (`private_code`, set in the launcher's Private room tab) | **Works on two instances**; not seen on real Steam |
| Bots, offline practice: Bots On + Solo + a card = BR against 5 AI Nightfarers, or Arena against one level-13 bot | **PASS on one PC**; Leonardo played both. Arena Duo / Trio against bots (LOCAL + bot allies vs a bot team, ARENA-6) built, not seen in game |
| **Arena 1v1:** Heolstor's arena, best of 3, level 15, Sparring loadout at max upgrade (saved loadouts at the Sparring Equipment), consumables and a full ultimate every round (one ultimate per round), round ends at 1 HP (no down), 3-minute clock, "One minute left!", 60 s overtime HP drain | **Played on real Steam** (2026-11-11/12/15/17, Leonardo + friends) on builds up to 0.3.x, which still had the get-up bug. The current rules (1 HP end, clock, overtime, one ultimate) PASS on one PC and against the bot (2026-11-17); **not yet seen on real Steam** |
| **Arena Duo / Trio (2v2, 3v3):** the round ends when a whole team is at 1 HP; a beaten player watches a living teammate (hidden, frozen; ARENA-6 replaced the bench seat); a leaver's team plays on (DECISIONS 2026-11-24) | **2v2 PASS on four instances** with the bench (2026-11-24, branch `arena-duo-trio`); the team spectate PASS on four instances played by hand (2026-11-25); Trio and real Steam open |
| Ultrawide: no 16:9 black bars, on by default (UW-1, `ultrawide = false` turns it off) | **Works on one PC** (2026-11-25, 2.39:1 borderless, HUD stays centred 16:9); branch `ultrawide` |
| Launcher app `NightreignPvP-Launcher.exe` (install wizard, Seamless Co-op via Nexus, auto-update of itself and the mod, Private room) and the version gate (an old mod or no version check = no start, LAUNCH-GATE) | **In use by players since v0.1.41**; LAUNCH-GATE's human acceptance is open (TASKS) |
| Orchestrator `nro` (puppets, keys, screenshots, scenarios, 2-3 instances) | **Works** ([orchestrator/SCENARIOS.md](orchestrator/SCENARIOS.md)) |
| Own networking (`net::bus`) | **Built** (NET-1/2); replacing Seamless Co-op is a study ([NETWORKING.md](NETWORKING.md)) |

**Not built yet:** the in-game password field (the launcher tab replaces it for now), Solo die-at-once with a revive
item (SOLO-REVIVE), the most-kills tiebreak.

**Known bugs:** the game crashes on quit (0xC0000374, heap corruption) on most runs since 2026-10-10, not proven to be Seamless Co-op's
(runs end with `nro stop`); some Arena ultimates deal no damage to a rival (ARENA-ULT-DMG).

## 2. What is deployed where

| Place | What |
|---|---|
| Players | **v0.4.0**, the latest release of `notgonnaleo/nightclash` (then `nightreign-pvp`; 2026-11-23, built by CI from `main` 92457fc): BR + Arena, 6 players with a 2-player minimum. Download page https://notgonnaleo.github.io/nightclash/ (Pages from `main` `/docs`; the old `/nightreign-pvp/` address is dead since the 2026-11-24 rename). Its patch notes still repeat v0.3.x's text |
| Old launchers (v0.3.3 and older) | Point at the deleted `aspherya/nightreign-pvp-releases`: the gate stops them ("Could not check for updates"). Leonardo tells players on Discord to download the new launcher once (DECISIONS 2026-11-23) |
| This checkout's `dist\` after `scripts\build.ps1` | Dev build for `launch.ps1` and `nro` |

## 3. Next

1. **A real-Steam Arena on v0.4.0** (both players' logs): the 1 HP round end under lag, clock and overtime, one ultimate per round, and
   ARENA-FS12's acceptance (rival's weapons, no mid-round respawn, one Commence to queue again).
2. **ARENA-ULT-DMG:** which ultimates do not hurt a rival, Nightfarer by Nightfarer.
3. **Next friend session with 4+ players:** FS7-1..8 on real Steam, MM-FILL and Duo / Trio (DUOTRIO-1). Collect every player's logs.
4. LAUNCH-GATE human acceptance (TASKS).

**Later:** GFX-1/2 (in-game UI), player count (HOST_MIGRATION), SOLO-REVIVE, MODE-SCOPE, MM-8, CLOUD-1..4 (log upload first),
normal expeditions (Leonardo has an idea). No match timer: the tide closes to a point.

## 4. Repository state

- **`notgonnaleo/nightclash`, public** (moved from `aspherya` on 2026-11-23; renamed from `nightreign-pvp` on 2026-11-24, GitHub redirects the old name: never create a repository called `nightreign-pvp` again). PRs go to `dev`; merging `dev` into `main` publishes a
  release by itself unless only docs, `scenarios/`, `docs/` or `design/` changed (RELEASE.md §6). Update `releases/notes.txt` first.
- **Only `dev` and `main` exist** (2026-11-23). Every older local branch (73, incl. unmerged ones such as `launcher-app`, `arena-0-spike`,
  `claude/nro-suite`, `claude/n6-headless-twins`) is in `..\nightreign-pvp-branch-backup-2026-11-23.bundle` next to the checkout;
  restore one with `git fetch <bundle> <name>:<name>`. Branch names in TASKS / testing-log refer to that bundle or to merged work.
- Tests: no CI on pull requests; run `cargo test --workspace`, clippy `-D warnings` (dev and `--no-default-features`) and fmt before
  merging. The release workflow runs the launcher and player-DLL subset again on `main`.
- Pins: game 1.03.3 (exe 1.3.3.0), Seamless Co-op v1.1.3, bundled me3 0.11.0 (`TECHNICAL_NOTES.md` §1, `pins/nightreign.toml`).
- Builds: `cargo build --release` = developer DLL (feature `dev`); `--no-default-features` = player DLL (refuses `dev_mode`). MSVC Build
  Tools needed (`D:\Programs\Microsoft Visual Studio`).
- Friend-session logs are gitignored (`*.log`); they live in `.claude/experiments/<session>/` on Leonardo's PC.
- `.mcp.json` registers the `nightreign` MCP server (`target\release\nro-mcp.exe`): `cargo build --release -p nro -p nro-mcp`.
