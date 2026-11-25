# How to use `nro` (the orchestrator): the practical guide

For the design and safety model see [README.md](README.md); for every scenario and the step vocabulary see [SCENARIOS.md](SCENARIOS.md);
for two real instances on one PC see [MULTI-INSTANCE.md](MULTI-INSTANCE.md) §18-§20. This page is "what do I type".

`nro` launches a **developer** copy of the game through me3 (never online, no EasyAntiCheat), drives it like a player (console, keys,
screenshots), fills it with AFK **puppet players** (rivals) and runs **scenarios** (a list of steps with checks). Every command prints JSON.
Everything it writes goes to `runs\<id>\` (gitignored).

## 1. One-time setup

| Step | Command (repo root, PowerShell) | Notes |
|---|---|---|
| Developer mod DLL | `powershell -NoProfile -File scripts\build.ps1` | `dist\nrpvp.dll`. Fails to copy while a game uses it |
| Game data | `powershell -ExecutionPolicy Bypass -File scripts\build-gamedata.ps1` | `dist\gamedata` (needs the 1.03.3 unpack in `Z:\Tools\nr-unpacked\1.03.3\` and Python). A new worktree has no `dist\gamedata`: run it there too |
| The tools | `cargo build --release -p nro -p nro-mcp` | `target\release\nro.exe`, `nro-mcp.exe`. Cannot link while `nro desk` runs: stop the desk first |
| The desk | `target\release\nro.exe desk` in a **normal terminal** | Needed for keys and screenshots (a process on your desktop); console and puppets work without it. Leave it open. The PC must stay awake, unlocked and untouched while a scenario runs |
| Steam | Running and signed in for single-game runs; **closed** for `mi-*` runs | The preflight checks it |

`nro env` shows the pins, the relevant processes and the current session. `nro` with no argument prints the command list.

## 2. A single game by hand

```text
nro launch --world --set mode="offline"   preflight, launch through me3, press through the title to the Roundtable
nro navigate expedition                   Roundtable -> the offline Solo expedition, off the hawk
nro status                                session state + the mod's `status`
nro cmd players                           any mod console command (CONSOLE.md §0); `nro cmd help` lists them
nro puppets 3                             3 AFK puppet players (1..5), they count as rivals
nro puppet_front 1.2                      put them side by side in front of you (metres)
nro puppet_ring 4                         or on a circle around you
nro cmd "puppet_hp 1 0"                   puppet in slot 1 to 0 HP = down (a sword swing does not reach every puppet)
nro cmd "night_status"                    what the match rules see: players, teams, who is out
nro keys "F; wait 2s; click 0.5 0.5"      keys and mouse to the game window (F interact, Q back, ALT off the hawk)
nro shot after_kill                       screenshot into runs\<id>\shots\after_kill.png
nro log 50 night_match                    last 50 mod-log lines containing "night_match"
nro stop                                  kill the game and me3 (always end a run with this; the game crashes on quit)
```

`--set key=value` overrides a key of the staged `nrpvp.toml` for this launch (repeat it). `--mod-dist DIR` uses another build folder instead
of this checkout's `dist\` (handy to test a worktree's build).

### Example A: "does a rival's kill defeat me and end the match?"
```text
nro launch --world --set mode="offline" --set defeat_exit_secs=15
nro navigate expedition
nro cmd "god 1"                  # so mobs do not kill an idle character while you set up
nro puppets 3
nro cmd "flag 10007420 1"        # offline solo carries a revive effect; this flag makes 0 HP a real death
nro cmd "rival_hit"              # the hit log says a rival hit me
nro cmd "hp0"                    # my HP to 0
nro cmd "night_status"           # expect: defeated by a rival, death held (about 15 s later: the defeat screen)
nro shot defeat
nro stop
```
This is exactly the `defeat-rival` scenario written by hand.

### Example B: "does the free Day 1 end?"
```text
nro launch --world --set day1_secs=120
nro navigate expedition
nro cmd "night_status"           # expect: free day 1 (the slowed clock runs up to 11:50)
nro stop
```

## 3. Scenarios (the normal way to test)

A scenario is a `.toml` in `scenarios\`: `[config]` keys written over the staged `nrpvp.toml`, then steps (`console`, `wait`, `expect`,
`wait_log`, `expect_log`, `mark`, `keys`, `navigate`, `puppets`, `screenshot`, `sleep_ms`). The run verdict is **PASS** (every `expect` /
`wait` held), **FAIL** (an `expect` missed) or **ABORT** (a `wait` timed out). One run takes about 3-6 minutes.

```powershell
nro scenarios                                              # list them (with their tags)
nro scenarios --tag smoke                                  # only the tagged ones
target\release\nro.exe run scenarios\day1-free.toml        # one scenario, full JSON report (runs\<id>\report.json)
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1               # every scenario except tags mi and lab; seconds per scenario and a total
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1 day1-free match-win   # only these
powershell -ExecutionPolicy Bypass -File scripts\regress.ps1 -Tag smoke    # only the scenarios with that tag
```

`regress.ps1` skips the `mi` and `lab` tagged scenarios unless you name them (or pass `-Tag mi` / `-Tag lab`). Tags (`tags = [...]` at the
top of a file): `smoke` (the quick sanity pair), `rules`, `hud`, `mi` (two instances), `slow`, `lab` (not a test). Run only what your change
can break; run all before a release ([../RELEASE.md](../RELEASE.md)). Which scenario proves which rule: [SCENARIOS.md](SCENARIOS.md).

**Reading the mod log in a scenario.** `wait_log` / `expect_log` read the mod's log FILE (`runs\<id>\mod\nrpvp.log`, the instance's own
in a multi-instance run), not the console's `log 300` tail, which can lose lines in event bursts (TRAPS.md "console tails"):
```toml
[[step]]
mark = "before"                      # remember the log's byte length now (per instance)
[[step]]
console = "hp0"
[[step]]
wait_log = { contains = ["defeated by a rival"], absent = ["panic"], timeout_s = 40, since = "mark:before" }
[[step]]
expect_log = { contains = ["start step HELD"], absent = ["RELEASED"], since = "run" }
```
`since` is `"step"` (default: from where the previous step ended, so a line the previous step itself logged is NOT seen: use `mark` before
the triggering command), `"run"` (the whole log) or `"mark:<name>"`. `absent` text appearing after the offset fails the step; in a
`wait_log` it aborts at once. `navigate = "commence_br"` is `commence` with the Battle Royale card selected first (`Up` wraps to the last card; the scenario `mi-br-card`). `navigate = "queue_br"` is the player's way into the Solo queue from the menu, with the card and Number of Players checked through the console `br_card` (needs `br_menu`; the scenario `mi-zero-click`). `navigate = "expedition"` ends with the console `god 1` (no wolf deaths); add `god = false` to that step
when a mob should be able to kill the character (`hp0` turns god mode off by itself).

### Writing a scenario (copy the nearest one)
```toml
name = "my-check"
about = "one line: what this proves"

[config]
mode = "offline"
mm_queue = false
ui_flag_watch = false          # its event bursts flood the 300-line console tail

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
expect = { console = "night_status", contains = ["held"] }
[[step]]
screenshot = "done"
```
Rules of thumb: reach a state with **commands, not keys** (`clock end`, `night_end`, `god 1`, `hp0`, `puppet_hp 1 0`, `flag <id> 1`); keys are
for the title screen and what has no command. A `wait` that times out ABORTS the run, an `expect` that misses FAILS it and the run goes
on. Add the new scenario to SCENARIOS.md and write a `testing-log/` entry when it passes.

## 4. Two real instances on one PC (`mi-*`)

For anything that needs two real games (matchmaking queue, item drops seen by both, rune sharing, HUD filter, the data channel).
Each instance gets an emulated Steam id (gbe_fork), so **Steam must be closed**.

```text
nro stop --all                                            # clean slate (waits until the games are really gone)
target\release\nro.exe run scenarios\mi-queue.toml        # both instances queue, meet, land in one expedition as rivals
target\release\nro.exe run scenarios\mi-rw3-items.toml    # RW-3: a defeat drops items, both screens see them
```
By hand: `nro launch --emulated 1`, `nro launch --emulated 2`, then any command with `--i N` (`nro cmd players --i 2`, `nro shot a --i 1`).

Requirements and limits:
- Steam closed; `nro` run from a **normal terminal**, not an agent's sandboxed shell (it cannot open the game process, so the preflight fails).
- About 7-10 GB of free RAM per game (two instances need roughly 20 GB). Four ran on 32 GB (2026-11-24, `mi-arena-duo`) with the
  settings of `scripts\four-instances.ps1 -Prepare` (closes Steam, Discord, WhatsApp; the shared `GraphicsConfig.xml`, also the real
  game's, to WINDOW 960x540, every quality LOW, the original saved once) and the page file carrying the rest. Afterwards
  `nro stop --all`, then `scripts\four-instances.ps1 -Restore` (puts the original back byte for byte).
  Four-instance run, in order: `four-instances.ps1 -Prepare`, `nro desk` (normal terminal), `nro run scenarios\mi-arena-duo.toml`,
  hands off the PC, `nro stop --all`, `four-instances.ps1 -Restore`. To double-click instead: `scripts\four-instances-prepare.bat` /
  `scripts\four-instances-restore.bat` (each runs the `.ps1` with its switch and waits for a key).
- Instance 4 (Steam id ...441) had no save: its first launch needed the new-game start played once by hand, up to the Roundtable.
- The emulator files come from the Nucleus handler folder (`D:\Programs\Nucleus\handlers\ELDEN RING NIGHTREIGN`, or the folder in `NRO_EMU_SRC`).
  Nucleus itself is not used or needed running, but that folder must exist.
- The Windows Firewall needs Allow-Private rules for the instance folders (once; MULTI-INSTANCE §16/§18).
- One run at a time: two sessions on one PC abort each other (TRAPS).
- Instances are `nightreign.exe` copies under `<drive>:\nro-instances\i1..i4`; `nro stop --all` ends all of them.

### Example C: "do both screens see the dropped gear?" (the `mi-rw3-items` flow)
```text
nro stop --all
nro run scenarios\mi-rw3-items.toml
# then open runs\<id>\report.json and the shots\ folder; the run log has the drop lines from both games
```
To explore by hand instead, run the lab `mi-rw3-drop` (keeps both games open, `teardown = "keep"`), then use `nro cmd "drop_gear" --i 2`,
`nro cmd "item_drop 0x... 1" --i 1`, `nro shot x --i 1`, and finish with `nro stop --all`.

## 5. Reading a result

- `nro run` prints JSON: `verdict`, `run` (the folder id), and per-step results. The full copy is `runs\<id>\report.json`.
- `runs\<id>\mod\nrpvp.log` is the mod log of that run; with instances there is one per instance (`-i1`, `-i2` suffix in the id).
- `runs\<id>\shots\` has the screenshots. Open them with the Read tool and look; do not trust a verdict without the picture for visual claims.
- Judge FAIL vs ABORT: FAIL = the game did something else than expected (read the log around the step); ABORT = it never got there
  (a menu did not open, a key was lost, the PC locked). `nro keys` into one of two windows can lose the first key: prefer commands.

## 6. The MCP server (for Claude)

`.mcp.json` registers `nro-mcp.exe` as the MCP server `nightreign`; every `nro` command above has a tool of the same meaning
(launch, navigate, cmd, keys, screenshot, puppets, run, stop...). Build it with `cargo build --release -p nro -p nro-mcp`; the path in
`.mcp.json` points at the main checkout's `target\release`. An agent drives a game exactly like the CLI does.

## 7. When something odd happens
- **"preflight failed"**: read the JSON; it names the failed check (a game or EAC process running, Steam in the wrong state, a hash that
  does not match `pins\nightreign.toml`, a dev `nrpvp.toml` that is not dev mode). Nothing was launched.
- **Keys do nothing / the screenshot is black**: `nro desk` is not running, or the PC is locked or asleep (`scripts\keep-display-on.ps1`).
- **The next launch crashes right after a `stop --all`**: wait a few seconds; `nro stop` already waits for the processes to end.
- **"only 0 of 3 puppets"**: `dist\gamedata` is missing or stale in this worktree; run `scripts\build-gamedata.ps1`.
- **Wolves kill an idle character in a long run**: `god 1` while you wait (the `defeat-rival` scenario does this before `hp0`; TRAPS notes one case where `hp0` is refused after god mode).
- More: [../TRAPS.md](../TRAPS.md) and [../TOOLS.md](../TOOLS.md).
