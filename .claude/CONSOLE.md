# Developer console

**Status (2026-11-01): CON-1, CON-2 and CON-4 done; CON-3 partly (`flag`, `hp0`, `god`).** The console works against a live game (SMOKE-1); the ImGui panel and the F-key
hotkeys are deleted (R5) and their remaining functions are console commands. `kill`, `down`, `bleed_clock` and `msg_show` are not built; CON-5 (in-game line, autoexec) are planned. Developer builds only (`dev` feature + `dev_mode = true`): the player zip never has it.

## 0. Use it now

Start a developer game (`.\scripts\launch.ps1`, or Nucleus after `scripts\deploy-nucleus.ps1`), then in any terminal:

```
dist\nrcon.exe list                      running games (Nucleus: index 0 = Instance0)
dist\nrcon.exe status                    the only running game
dist\nrcon.exe -i 1 players              game 1 from `list`
dist\nrcon.exe --all "echo hello"        every game
dist\nrcon.exe                           interactive prompt (exit to leave)
```

Every command and its reply is also logged (`console: <line> -> <reply>`). From Git Bash, a leading `/` gets rewritten into a path:
leave it out there (`kill`, not `/kill`).

### Commands available now

The source of truth is the registry in `crates/nrpvp/src/console/mod.rs` (`help` prints the same list).

| Command | Args | What it does |
|---|---|---|
| `help` | `[name]` | List the commands, or one command's help |
| `echo` | `text…` | Write `console: echo: text` to the log (a note for whoever reads the log) |
| `mark` | `[text…]` | Write a numbered `MARK #n: text` line to the log |
| `log` | `[n]` | The last n log lines (default 20, at most 300), straight from the game |
| `status` | | World loaded or not, local player and its team value, PvP mode, game mode (`mode: br` / `mode: offline`), matchmaking line, pid |
| `mode` | `[br\|offline]` | OFF-1 game mode. `br`: Commence starts the queue. `offline`: Commence with Solo starts the game's own solo expedition at once (Duo/Trio say "solo only"); switching to it leaves the queue. Asked during an expedition, the change waits for the Roundtable. No argument shows the mode. Logs `mode: <mode>` |
| `bots` | `[10\|30\|50\|10000]` | OFF-4, offline mode only: sets our flag so the game's own NPC bot instruction `2003[97]` runs with `NPCBotTableParam` table 10 / 30 / 50 (2 Nightfarers, level 1: heroes 1+2, 3+4, 5+6) or 10000 (2 of the 8 base Nightfarers, level 6; the game holds 2 bots). No argument: the last request and the bot manager's state. Logs `bots:`. Dev only |
| `bots_fill` | `<0..5>` | BOT-1, offline mode: fill the expedition with this many hostile bots (more rounds when already filled; the count stays for later expeditions; config `bots_fill`). Logs `bots:`. Dev only |
| `bots_list` | | BOT-1: the bot roster: stage, kept, alive; per bot handle, alive / dead / gone, HP, team, position. Dev only |
| `bots_switch` | `[on\|off] [solo]` | BOT-5: the Bots switch of Matchmaking Settings (the expedition menu must have been open once); `solo` also sets Number of Players to Solo; no argument reads it. Dev only |
| `bots_kill` | `[n\|all] [mob]` | BOT-2 test aid: HP 0 on living rival bot n (1-based) or all, last attacker written as LOCAL (out of the match) or with `mob` as a made-up enemy (replaced after 14 s). Dev only |
| `bots_scan` | | BOT-0 spike: every bot-like character (npc 600040000, invaders 6000300x0, set 0x0E) in every WorldChrMan set. Dev only |
| `bots_gate` | `[hold <ms>]` | BOT-0 spike: the bot instruction's solo gate (= the session's players); `hold` keeps it at 1 for ms, then restores it. Dev only |
| `bots_clear` | | OFF-4 spike, offline mode only: runs `2003[98]`, which removes every bot the game spawned. Dev only |
| `clock` | `[night\|circle\|end\|hh:mm[:ss]]` | M5: the expedition's play-area clock (`clock: 14:03:10 (running, speed …); circle radius 527.641 m (day 2: first 367.7, final 0.0; day 1 final 0.0)`: the circle's radius now and the game's stage records, TIDE-MAX, MEMORY_MAP §16). With a time the game jumps its own clock there on its next frame (its set-time mailbox, MEMORY_MAP §16): `circle` = 20:20 (final circle starts 20:24), `night` = 22:55 (night events from 23:00), `end` = 23:58:30 (night flag at 23:59). Expedition only. Logs `clock:` (one line per game hour, and "the game applied …" after a jump). Dev only |
| `night_end` | | M5-3: end the current night now. Sets our flag 10007400, which the night-end events (`gamedata/emevd-nights.json`) wait for besides their 10 min fallback; the game's own morning follows (flasks, grace, next day; no heal since M5-5). Acts only at night and only in the host's game; without our event file (Nucleus) the night match writes 7502 itself 1.5 s later (night 1 only). Logs `nights:`. Dev only |
| `night_status` | | The match rules in this game: `free day 1` / `day n` / `night n` / `match over` (`decided (winner …)`), players, teams seen, how many are out, `LOCAL in` or `defeated by a rival / the tide`, mob deaths, recorded respawn places, and `death held` (RW-1). Answered on the pipe thread. Dev only |
| `grace_lock` | | GRACE-1: `ON, Sites of Grace are off` while a PvP fight locks the graces (game flag 7525), else `off`. Pipe thread. Dev only |
| `killfeed` | | KF-1: how many killfeed lines this game showed, and the last one. Pipe thread. Dev only |
| `score` | | SB: every participant's kills and deaths this match as this game counted them (`name K n D n [out]`). Pipe thread. Dev only |
| `score_show` | | SB: show LOCAL's own score message now, as opening the pause menu does. Pipe thread (the main loop shows it next tick). Dev only |
| `score_dialog` | `<text>` | SB research: write `<text>` (`|` = a new line) over the end-of-match summary's text; `flag 10007110 1` then shows it (Roundtable only). Pipe thread. Dev only |
| `toast` | `[text...]` | KF-1: show this line as one of the game's toasts (takes the game's markup); no text = counters. Pipe thread. Dev only |
| `toast_icon` | `<id>` | KF-1 research: icon id of the next toasts (`UserDispLogParam`), 0 = none. Pipe thread. Dev only |
| `night_count` | `[0\|1]` | Test switch: 1 (default) the match ends when one team is left standing; 0 it never ends by count. Resets at launch. Dev only |
| `spectate` | `[0\|1]` | RW-1: 0 = a defeated player's held body stays where it fell (shows what the game itself does for a dead player), 1 = it follows a living player (the mod's spectator; the `spectate` setting's default). Dev only |
| `rune_watch` | `[0\|1]` | RW-8 research: log which instruction writes LOCAL's rune count (`PlayerGameData+0x6C`, hardware write watch) as `T-041 bar_trace` lines. Dev only |
| `rival_hit` | | RW-1 test aid: record a rival's hit on LOCAL now; a death within 30 s of it is a defeat for good, a death without it is a normal one (offline: `flag 10007420 1` first, so `hp0` is a real death). Dev only |
| `memscan` | `[utf16\|ascii\|hex] <text>` | Research: scan the whole process for a string or bytes on a background thread (about a minute); hits are logged as `memscan:` with 16 bytes before and 80 after; no argument shows the last result. Dev only |
| `maphide` | `[0\|1\|auto]` | RW-9: 1 = empty the map / compass list of other players now, 0 = show them, auto (default) = hidden while a Solo match runs (PvP on, team size 1, in an expedition) and only when `rivalhud` could not hook the map site (fallback); no argument shows the state (the list's item count, hidden or not) |
| `ultrawide` | `[0\|1]` | UW-1: 1 = no 16:9 black bars (one byte at `exe+0x1B8B286`, `game/ultrawide.rs`), 0 = vanilla letterbox; the game uses it the next time the display settings are applied (change the window mode or resolution in its menu). No argument shows the bytes. Config `ultrawide` (true by default) patches at load. Dev only (the config key works in both builds) |
| `rivalhud` | `[0\|1\|auto]` | RW-9: rivals off the map pin, compass mark, party bar and name tag, and the **map markers (pins) a rival places** off the map, compass and 3D world, green beam included (six call-site hooks, `game/hudfilter.rs`); the lock-on target keeps no name tag (the game's own enemy bar names it). auto (default) = while a match runs; no argument shows the state, the rival handles and map ids, the lock-on target, per site `hiding a rival now` with counters, and `markers: records [owners], map list [owners]` (pins received vs pins the map / compass shows) |
| `runeshare` | `[0\|1\|auto]` | RW-8: kill runes from a rival's kill are not credited to this game's player (death-credit hook `exe+0x3DE800`, MEMORY_MAP §21); 1 = force on, 0 = force off (vanilla share), auto (default) = while a match runs. No argument: state, `runes N (+d since mark)`, per-site counters (credited / blocked, message credits seen). Both builds' hook; the command is dev only |
| `runes_mark` | | RW-8 test aid: remember this game's rune count now; `runeshare` then shows the change since. Dev only |
| `mobs` | | RW-8 test aid: the nearest living enemies loaded on this game (handle, npc id, HP, distance; found through the `ChrSet` / `OpenFieldChrSet` vtables in `WorldChrMan`). Dev only |
| `mob_go` | `[hp] [skip]` | RW-8 test aid: put this game's player 1.5 m from the nearest living enemy (or the skip-th nearest), facing it; with hp, write that enemy's HP first (1 = one hit kills; sticks only on the enemy's owner game). Dev only |
| `mob_kill` | `[skip]` | RW-8 test aid: the nearest living enemy (or the skip-th nearest) gets LOCAL as its last/current attacker (`+0x198` / `+0x1A0`, what a real LOCAL hit fills) and HP 0, so this game's death credit runs as after a real kill; the other game sees no attacker (the filter's `Unknown` case). The deterministic stand-in for the click-based kill (`mi-rw8-runes`). Dev only |
| `tp_player` | `<slot 1..5> [dx dz]` | RW-9 test aid: put this game's player next to the player in that player-list slot (default 3 m along x, offsets at most 50 m), any distance, facing it. Dev only |
| `padpoll` | (none) | Research: every virtual input bit of the game's own pad that is set now (hold a button, call it, diff against idle), and whether Start / d-pad groups read down. Dev only |
| `mutex` | | Multi-instance (orchestrator): close this game's handle to the engine's single-instance mutex `Global\SekiroMutex` now (config `allow_multi_instance = true` does it at load), so the next game instance can start; the reply lists the named mutexes seen. Dev only |
| `hp0` | | Test aid (2026-10-24): LOCAL's HP is written to 0 on the next tick (the data-module field of T-040; god is turned off). In a session it downs; with `flag 10007420 1` (die at once) it is a real death even offline. Dev only |
| `god` | `[0\|1]` | Test aid (2026-10-23): 1 = this game's player is refilled to full HP every tick while up (one hit bigger than full HP still downs). Team values are untouched. Off at launch. Dev only |
| `flag` | `<id> [0\|1]` | M5-4 research: read any event flag (`flag 7005`); with a value, write one of ours (10007000..=10007999) or the game's 7000 (game clear), 7001 (game over), 7502 / 7507 (night 1 / 2 done), 7508 / 7510 / 7521-7523 (Day 3 chain, ARENA-0), 7525, in this game only. ARENA-0: `flag 10007500 1` warps this game's player into Heolstor's arena (event 99100900, dev game data only). For Nucleus runs, where our event files are not loaded. Logs `flag:`. Dev only |
| `death_trace` | `[0\|1]` | M5-8 research: while this game's player is dead (HP 0, not downed), breakpoints on the later comeback steps (`exe+0x5C3F80` begin respawn, `exe+0x5C4360` respawn; the start step is `death_hold`, M5-9) log when each runs, its thread and arguments; LOCAL's HP, state, team, `+0x1F4`, SpEffects of interest and position every second. Logs `M5-8`. Dev only |
| `hitkill` | `[0\|1]` | test aid: 1 = any enemy within 80 m of this game's player that lost HP dies at once with this player as the killer (works on the enemy's owner game, the host's). Dev only |
| `menugate` | `[0\|1]` | RW-1 research: 1 = the Esc key asks the game to open the pause menu even while LOCAL is dead (sets `CSMenuMan` ctrl+0x179); 0 = left to the game (the night match does this by itself for a defeated player) |
| `trace_write` | `<addr>\|0` | research: log which instruction writes the 4-byte aligned address (hardware write watch, slot 3) with registers and a stack excerpt as `T-041 bar_trace` lines; 0 stops. Heap addresses change every launch |
| `trace_read` | `<0xaddr>\|0` | RW-5 research: like `trace_write` but the watch also fires on READS; 0 stops (and goes back to write-only) |
| `death_hold` | `[0\|1]` | M5-9: manual switch of `game/deathhold.rs` (the module the Solo night match arms by itself; it is in the player build too). 1 = a dead player stays dead (the start step `exe+0x5C4810` returns at once; no fade). 0 = if a step is held, it is called now through gate A `exe+0x5BCE10` (gate B `exe+0x675480` armed too after 2 s) and the game's own comeback follows. No argument shows the state, the counts and the last thread. Logs `M5-9 deathhold`. Dev only |
| `speffects` | | M5-8 research: this game's player's SpEffect ids, and the ones of interest (revive items 540155 / 6999100 / 6999500 / 8970061, die-at-once 102130, night 102120). Answered on the pipe thread. Dev only |
| `players` | | Every player in this game's ChrSet: slot, `pN`, id, team value, relation (me / ally / rival) |
| `team_set` | `<value> [target]` | Write team_type on this game's copy of the target's players (default `me`) |
| `team_restore` | `[target]` | Put back the team_type captured before the first write (default `me`) |
| `team_persist` | `0\|1 [target]` | Keep re-writing the last written team_type on the target's players (1), or stop (0) |
| `pvp` | `0\|1` | Resume (1), or pause and put back every player's captured original (0; the old F12) |
| `mm_queue_solo` / `mm_queue_duo` / `mm_queue_squad` | | Join that matchmaking queue |
| `mm_leave` | | Leave the queue or the ready match |
| `mm_status` | | The matchmaking status line |
| `mm_join` | `link\|invite\|direct\|unlock_public\|unlock_invisible\|relock` | One join-method attempt for the ready match (dev, `mm_join = true`) |
| `mm_sc_create` | | Create a lobby the game's way; does Seamless Co-op adopt it? (dev, `mm_sc_create = true`) |
| `mm_quiet_test` | | Suppress the Steam invite window for 60 s (dev, `mm_quiet_invite = true`) |
| `revive_fail` | `[n]` | T-040 test switch: the next n rival re-down tries fail ("forced by console") without reading memory; no argument shows how many are left |
| `early_redown` | `[0\|1]` | T-040 follow-up experiment: knock a rival-revived player down during the get-up (1) instead of after it (0, default); no argument shows the state. Dev only, resets at launch |
| `bar_trace` | `[0\|1]` | T-041 step 1 trace: while LOCAL is downed, a hardware write watch (slot 3, only if free) logs which instruction writes the revive bar, as `T-041 BAR WRITE #n: rip=exe+0x… value=… tid=…`; no argument shows ON/OFF and the hit count. Dev only, resets at launch |
| `hawk_env` | | HAWK-3: logs the env object (`exe+0x3C1D3D8`) and whose code its switch at `+0x10` is (game RVA, ours, or another module's). Reads only. Dev only |
| `hawk_trace` | `[0\|1\|all]` | HAWK-3: swaps the env switch for our pass-through and logs every change of the hawk answers 377/384/394/395 per character (`HAWK-3 0x… (LOCAL) env 377 0 -> 4`); `all` also logs every other id the rider asks (noisy ids are muted). No argument shows the state. Dev only |
| `hawk_drop` | | HAWK-3: answers `GetBirdEventID` = 2 ("route ended") to the rider for 1.5 s, so the start hawk drops the player. Dev only |
| `hawk_ride` | `[tree\|start] [secs] [rise]` | FS7-5: start a hawk ride for LOCAL now (377 answered 1 tree / 4 start once; only tree works outside the match start), after lifting LOCAL `rise` m (default 0); a tree ride still on after `secs` (default 30) is dropped. Dev only |
| `hawk_tree` | `0\|1` | HAWK-3: start-hawk rides (377 = 4) answer 1 (tree hawk) instead; set before the ride. Acts on every rider (solo tests only). Dev only |
| `hawk_cruise` | `0\|1` | HAWK-4 (on by default since HAWK-2: `[hawk] enabled` in `nrpvp.toml`; this overrides it for the session): start rides run as tree rides (answer 1 for 4) with 394 answered 0 (stick hidden), so the hawk holds the route; once the real 394 reaches the dive hold, the stick passes through and 384 is capped at 1999 for the dive time, then the vanilla off-route fall. Set before the ride. Log lines `HAWK-4 phase …`. Dev only |
| `hawk_dive` | `[hold] [ms]` | HAWK-4 (defaults from `[hawk] dive_hold` / `dive_secs`): dive trigger in 394 units (default 400; 394 rises about 1,600 per second of stick) and dive length in ms (default 7000 since 2026-10-22); no argument shows the state. Dev only |
| `hawk_start` | `0\|1\|2` | HAWK-4: ride start under cruise/tree: 0 shows the tree start animation, **1 cuts it** (default: `IsAnimEnd` answered TRUE for 0.4 s), 2 keeps the vanilla start and switches to the tree ride 2 s later (with the cut). Dev only |
| `hawk_boost` | `0\|1` | HAWK-4: during a dive the script's hawk speed goes to x1.3 (`GetSpEffectID(99230)` answered TRUE on the speed check only; default 1). Dev only |
| `hawk_off` | | HAWK-3: everything off (the HAWK-2 cruise too, until the next launch) and the game's switch put back. Dev only |
| `revive_block` | `[0\|1]` | T-041 prototype: while LOCAL is downed, a hardware breakpoint on `exe+0x675159` clears the "revive completed" flag when the reviver is a rival, so no get-up starts; no argument shows ON/OFF and the block count. **On by default** (both builds run the block); the command only switches it off for a comparison run. Dev only |
| `pos_next` / `pos_save` / `pos_go` | | Snap-back test (Task D): select a candidate, save your values, write them back (you only) |
| `arena_loadout` | `[apply]` | ARENA-3: the loadout captured at the Roundtable for the Arena (hand-slot weapons with their custom row and skill, goods); `apply` gives it now as the arena spawn does (SetLevel 15, every weapon at max upgrade into its slot with its skill, the goods the expedition took, recovery), e.g. in a normal expedition for a solo check (`game/arenaloadout.rs`). Dev only |
| `arena_clock` | `[0\|1]` | ARENA-CLOCK: the Arena round's seconds left shown in LOCAL's rune counter (top right) while an Arena match is on (`game/arenaclock.rs`); 0 hides it and puts the rune count back, 1 shows it (default). Dev only |
| `arena_hide` | `[0\|1]` | ARENA-6: a beaten player who watches its team (the body follows a living teammate) is also disabled once on its far bench (1, default: flag 10007522, Change Character Enable State; not drawn on its own game, the others keep seeing it on the bench because its position stops reaching them, mi-arena-hide-lab 2026-11-25) or only a ghost with no collision and no gravity that follows at once and is seen on the teammate by everyone (0: flag 10007521 alone). Dev only |
| `arena_limit` | `[secs]` | ARENA-3: the Arena round time limit (default 180 s); a value sets a shorter one in this game for tests, 0 = the default. At the limit the overtime starts (60 s of HP drain, ARENA-OT); the host judges and publishes the verdict (`net::bus` ARENA_ROUND). Dev only |
| `give` | `<id hex> [qty] [slot]` | ARENA-3 research: add an item to LOCAL's inventory with the game's add-item call `exe+0x1F05F0(PGD+0x1D8, &id, qty, …)` and, with a slot 0..5 (L1 R1 L2 R2 L3 R3), equip it with `exe+0x1F3430`; both run **on the game's thread** (carried by `itemdrop`'s breakpoint; a direct call crashed the game, TRAPS). A weapon not in a hand slot does not show in Nightreign. Logs `give:`. Dev only |
| `gaitem` | `[handle hex]` | ARENA-3 research: dump the game's item instance (`CSGaitemImp`, MEMORY_MAP §29) of a handle, or of each hand slot's weapon, as `u32` words. Pipe thread, read only, logged as `gaitem:`. Dev only |
| `inv` | `[mark\|diff]` | ARENA-3 research: LOCAL's level (`PGD+0x68`), runes, equipped weapon ids and every inventory entry; `mark` keeps `PlayerGameData` (0x1000 bytes) and the inventory, `diff` lists every changed word and the entries that came, went or changed (`game/invspy.rs`). Pipe thread, read only, full lists in the log as `inv:`. Dev only |
| `rival_gear` | | ARENA (2026-11-13): the hand slot ids LOCAL holds and, for each other player, the copy this game holds of theirs (`GameDataMan+0x10` slots, 0x8A0 each), which is what it draws. A rival's change shows here only after its game broadcast its player data (`exe+0xDE20E0`). Pipe thread, read only |
| `pos` | | Orchestrator (2026-10-27): this game's player in world metres (physics module minus origin, MEMORY_MAP §8), the physics position, the heading quaternion at `Physics+0x50` with its `yaw`, and `menu` = `CSMenuMan+0x48` (0 = no game menu, `0x200004` = the expedition menu). Answered on the pipe thread, read only. Dev only |
| `tp` | `<x> <y> <z> [yaw]` | Orchestrator: put this game's player at that world position (at most 100 m per call), with a yaw also turned to that heading (written only over a value that reads as a heading). `nro navigate invite_members` uses it to stand the host at the Roundtable table. Logs `teleport:`. Dev only |
| `puppet_arm` | `[0\|1]` | ORCH-2: 1 = the NPC bot manager's next characters go into the **player list** as AFK puppet players (offline mode, a world loaded; then `bots 10`, 2 per request). Breakpoints only: the creator call `exe+0x38F2B3` (target set swapped), `exe+0x38F2EE` (the manager forgets them), the destroy function `exe+0x51B2B0` (Seamless Co-op's sweep is skipped for puppets). nightreign-orchestrator `nro puppets <n>` does the whole sequence. Logs `puppet:`. Dev only |
| `puppet_max` | `[n]` | ORCH-2: at most n puppets (0..5); further creations are skipped while armed. Dev only |
| `puppet_list` | | ORCH-2: every entry of the game's player list: slot, address, vtable, handle, id, npc id, team, HP, downed state, world position, near-death SpEffect. Dev only |
| `puppet_move` | `<slot> <dx> <dz>` | ORCH-2: put the puppet in that player slot at LOCAL's position + (dx, dz) m (physics position write, at most 50 m). Dev only |
| `puppet_hp` | `<slot> <hp>` | ORCH-2: write a puppet's HP (character data module); a hit at 1 HP downs it. Dev only |
| `puppet_near_death` | `[0\|1]` | ORCH-2: 1 (default) = a puppet holds SpEffect 8970061 while up (0 HP downs instead of killing) and loses it while downed (bleeds out instead of the solo auto-revive). Dev only |
| `net_peer <steamid64>`, `net_ping [n]`, `net_status` | | NET-1 (config `net = true`): add a roster peer to the data channel, ping every peer n times, counters (sent, recv, pongs, rtt). Dev only |
| `net_verdict <boss> <team>`, `net_publish <topic> <key> <value>` | | NET-2: publish a fact (topic BOSS_VERDICT=1 for `net_verdict`) to every other player; each machine records it once (first value per key wins), resent until acked. `net_status` shows `facts_out/acked/in`, `fact_conflicts`, `last_fact`. Dev only |
| `orb_auto [0\|1]`, `orb_list`, `orb_off [key]`, `orb_on [key]`, `orb_claim [key]`, `orb_go [key]` | | RW-5: boss reward orbs (Dormant Power) found in MapItemMan by content with the claim book; off nulls the orb slot (keeps the pointer), on restores it only if the slot is still 0 and the record unchanged, claim = LOCAL's team claims it and publishes ORB_CLAIM, go = stand 1.2 m from it (no distance limit). No key = the only orb. **`orb_auto 1` first**: the rule is off by default (vanilla). Dev only |
| `boss_orb [go\|hp]` | | RW-5 test aid: the strongest living Dormant Power boss (same on every game, never an expedition-ending one); go = stand next to it, hp = HP 1 (host game only) |
| `boss_list` |  | RW-5 test aid: the living bosses (HP above 1000) with handle, npc id, HP, position and distance. `mobs` skips them. Dev only |
| `boss_go` | `[skip]` | RW-5 test aid: put this game's player 1.5 m from the nearest (or skip-th nearest) boss, facing it. Dev only |
| `boss_kill` | `[skip]` | RW-5 test aid: the nearest boss gets LOCAL as last attacker and HP 0, so this game runs the boss death as after a real kill (the other game sees no attacker, MEMORY_MAP §21). Dev only |
| `boss_hp` | `<n\|0xhandle> <hp>` | RW-5 test aid: write a boss's HP on this game (1 = one real hit kills it; sticks on the owner game). Dev only |
| `boss_info` | `[n]` | RW-5 test aid: boss n of `boss_list`: address, vtable and its NpcParam reward lot fields. Dev only |
| `boss_lots` | `<n> <field> <value>` | RW-5 test aid: write one NpcParam reward field of boss n on this game. Dev only |
| `drop_gear [dry]` | | RW-3 test aid: `dry` lists what a rival's defeat would drop (LOCAL's equipped weapons, talismans and consumables of the inventory, never the flask) and whether this game is the host; without `dry` it drops them now at LOCAL's spot without a defeat. Dev only |
| `arena_card` | `[0\|1\|hook\|unhook]` | ARENA-0 (MEMORY_MAP §28): no argument shows the Everdark rotation list (`CSServerInterface+0x30`) and the hook state; `1` / `0` add / remove byte 7 (row 17 as an Everdark card: hidden offline); **`hook` = an execute breakpoint (hwbp slot 1) on the card list loop that adds row 17 "Arena" as a normal card when the expedition menu opens**, `unhook` frees the slot. Logs `arena_card:`. Dev only |
| `item_drop <id hex> [count] [dx dy dz]` | | RW-3 E1 research: drop one item with the game's own drop function `exe+0x576110` at LOCAL's position (+ offset), reply with the MapItemMan slots that changed. Dev only |
| `session` | | HM-0 research: role, protocol state, limit, host and players of this game's `CSSessionManager`, with raw qwords. Read-only, dev only |
| `mobs_raw [n]` | | HM-0 research: the n nearest living enemies (default 4) with position and the raw network fields of their `ChrIns`. Read-only, dev only |
| `hm_leave_lobbies` | | HM-1 research: this game leaves every Steam lobby it has seen, so the lobby owner moves as on real Steam when a host dies. Dev only |
| `mobs_dump` | | HM-1 research: every enemy this game holds, sorted by handle, into the log (`handle:npc:hp:x,z`), with a hash in the reply, to compare two games. Read-only, dev only |
| `grace_go [skip] [npc]` | | GRACE-1 test aid: stand 1.5 m from the nearest Site of Grace. Dev only |
| `grace_status` | | GRACE-1 test aid: LOCAL's HP and flag 7525. Dev only |
| `hp_set <n>` | | GRACE-1 test aid: write LOCAL's HP. Dev only |
| `ult` | `[empty\|full]` | Arena test aid (MEMORY_MAP §11 "Ultimate charge"): LOCAL's Ultimate Art charge now / needed in `[[PlayerIns+0x1B8]+0x148]`, or set it empty / full. Logs `ult:`. Dev only |
| `players_pos` | | HM-0 research: world position of every character in the player list, to compare what two games see. Read-only, dev only |
| `mm_open_lobby` | | MM-3g: the game makes its multiplayer lobby now, as Invite Members does, with no menu and no Steam window (next frame, game thread). Dev only |
| `expedition_start` | `[leader] [players byte] [target]` | MM-7: this player is ready, as Commence does in a party; with `leader` it sends the settings first. Players byte 0 by default, target row, negative for none. Dev only |
| `br_card` | | UI-3 test aid: whether the Battle Royale card is the chosen target of the expedition menu, and its Number of Players byte (needs `br_menu`). Dev only |
| `exec_trace` | `<slot 1..3> <rva hex\|0>` | research: log every run of `exe+rva` with registers, the code addresses on its stack and the memory `rdx` points at (hardware execute breakpoint, one slot; check the slot is free); 0 frees it. Logs `exec_trace:`. Dev only |
| `rawcall <rva hex> [arg1..arg4]` | | NET-5 / ARENA-3 research: call `exe+rva(args)` once on the game thread (up to 4 integer arguments: decimal, `0x` hex, or `pgd` = LOCAL's `PlayerGameData`), reply with rax. `rawcall 209E20 pgd 15` = SetLevel (MEMORY_MAP §29). Dev only |
| `puppet_think`, `puppet_forget`, `puppet_session`, `puppet_watch`, `peek` | | ORCH-2 research: NpcThink id for new puppets (default -1, no AI); bot manager forget; the session-field experiment; a write watch / execute trace; a read-only hex dump (`peek wcm+0x9A40 0x40`). Dev only |
| `pad`, `pad_connect`, `pad_clear`, `pad_status` | `A; wait 500; LS 0 1 800` | ORCH-1: a virtual XInput pad 0 (import-table hook of `XINPUT1_4` ordinal 2). **Not effective yet**: the game only polls XInput pads it found in a device scan, which does not run without a real pad. Dev only |
| `whoami` (pipe only) | | `pid <pid> <DLL folder>`; `nrcon list` uses it |

Targets: `me`, `all`, `allies`, `rivals`, `p1`…`p6`, `s0`…`s5` (§3.1).

---

## 1. Where the weight actually is

You asked for "an in-game command line, simpler and lightweight". Two parts of that need correcting first:

- **The heavy part is not the panel, it is the renderer.** To draw anything over the game (a panel or a console line) we hook DirectX 12
  through hudhook/ImGui. A CS2-style in-game console would keep exactly that hook. Turning buttons into a text box saves code, not weight.
- **The in-game overlay does not draw under Nucleus** (STATUS traps). Nucleus is where every multi-player test on your PC runs. An
  in-game-only console would be blind in the setup you use most.

So the valuable part is the **command layer**: one place that turns `kill`, `team_set 29`, `mm_queue_solo` into actions. The window you
type in is just a frontend. We already have half of the layer: `overlay::shared::Command` is the list of actions the panel's buttons
send to the main loop.

## 2. Proposal

```
 frontends                          command core (dev feature)                 executors
┌───────────────────────┐          ┌───────────────────────────────┐          ┌────────────────────────┐
│ nrcon.exe  (pipe)     │──text──► │ parse → look up in registry → │──Cmd───► │ main loop (game memory)│
│ in-game console (F1)  │──text──► │ check args → build Command    │──Cmd───► │ Steam thread (mm)      │
│ autoexec.cfg          │──text──► │ reply text ◄──────────────────│◄─reply── │ console itself (help)  │
└───────────────────────┘          └───────────────────────────────┘          └────────────────────────┘
```

**Frontend 1, first: external console over a named pipe.** The DLL opens `\\.\pipe\nrpvp-<pid>` (local user only). A small
`nrcon.exe` attaches to one or all running games:

```
nrcon                      # interactive, picks the only running game
nrcon -i 1 "kill"          # Nucleus instance 1
nrcon --all "status"       # every running instance
```

Why it comes first:
- It works under Nucleus, in both windows at once, with no focus juggling (the END-key trap goes away).
- **Claude can drive it.** An agent can type `status`, `players` or `mm_status` into a running game and read the reply, instead of
  asking you to press F9 and paste a log. That removes human steps from every RE and test loop, and is the biggest payoff in this
  document.
- No rendering, no input capture, nothing new hooked in the game.

**Frontend 2, later: in-game console.** A one-line ImGui input plus the log tail, toggled with a key (CS uses `~`; SC already binds F4).
It reuses hudhook in dev builds only. Nice for solo play; not needed for testing.

**Frontend 3: `autoexec.cfg`.** Commands next to the DLL, run on world load (`mm_queue_solo`, `rival_hit_seconds 2`). It makes
repeatable test setups a file instead of instructions.

## 3. Command shape (design; built commands are in §0)

CS style (Leonardo, 2026-10-20): **a command name is one word with underscores, never spaces** (`mm_queue_solo`, `team_set`,
`bleed_clock`). Only values follow after a space, as in CS (`sv_cheats 1`, `team_set 29 all`). `;` chains commands, quotes wrap text
with spaces. A setting ("cvar") is typed by its own name: alone it prints the value, with a value it sets it (`rival_hit_seconds` /
`rival_hit_seconds 2`). Every command has a help line and runs on exactly one executor.

| Command | Does | Executor | Exists today as |
|---|---|---|---|
| `help [name]` | List commands / one command's help | console | — |
| `echo "text"` | Write `MARK #n: text` to the log (your `console.log`) | console | `Command::Mark` |
| `status` | Build, world state, player count, loop time, queue state | main loop | heartbeat line |
| `players` | ChrSet table: slot, id, team, HP, downed, rival/ally | main loop | overlay table |
| `team_set <v> [slot\|all]` / `team_restore [slot\|all]` | Write or restore team_type | main loop | `Set`, `SetAll`, `SetFor`, `Restore*` |
| `pvp 0\|1` | Pause or resume the automatic rival marking | main loop | `SetPvpve` |
| `kill` | Local player dies (§7 Q2) | main loop | — (HP 0 = down only) |
| `down` | Local player goes down (HP 0) | main loop | rule v1's write |
| `bleed_clock <s>` | Set own bleed-out seconds while downed | main loop | (was nrscan `poke`, deleted) |
| `mm_queue_solo` / `mm_queue_duo` / `mm_queue_squad` / `mm_leave` / `mm_status` | Matchmaking | Steam thread | `MmQueue`, `MmLeave` |
| `flag_get <id>` / `flag_set <id> 0\|1` | Read or set an event flag | main loop | `game::flags` |
| `msg_show <k>` | Show matchmaking message k in game | main loop | `ui_msg` flags |
| `<setting> [value]`, `cvar_list` | Read or change a live setting (`rival_hit_seconds`, intervals) | main loop | toml only, restart |
| `pos_save` / `pos_go` | Snap-back test (T-028) | main loop | F7 / F6 |
| ~~`dump_players` / `discover`~~ | Dropped: R5 deleted the discovery scanner and the dumps (findings are in MEMORY_MAP) | — | — |

`kill` and every other write keep today's gates (PlayerIns vtable check, own client only).

### 3.1 Targets (Leonardo's idea, 2026-10-20)

Player commands take an optional target, CS style: `kill`, `kill p2`, `team_set 29 rivals`, `down all`. Default is `me`.

| Target | Means |
|---|---|
| `me` | The local player (ChrSet slot 0) |
| `p1`…`p6` | A player by session number: `p2` = id `0x6002…` (MEMORY_MAP §3), the same on every game once the expedition has loaded |
| `s0`…`s5` | A ChrSet slot on *this* game (differs per game; for debugging) |
| `allies` / `rivals` / `all` | From the current roster |
| a name | **Later.** No name is located in memory yet; it needs the Steam ID ↔ player mapping (MM-6). Until then `players` shows ids |

**The catch: some writes only work on the owner's game.** HP, the bleed-out clock and the downed rule only take effect on the
victim's own client (MEMORY_MAP §5–6); `team_type` is per client by design. So:
- `team_set`, `team_restore`, `players`, reads: run locally against any target.
- `kill`, `down`, `bleed_clock` on someone else: **routed by `nrcon`**, not by the game. `nrcon kill p2` asks every running instance
  for its local player id and sends `kill` to the one whose `me` is `p2`. That covers Nucleus and anything else on this PC. Inside the
  game, `kill p2` answers "p2 is not this game's player; use `nrcon kill p2`".
- Across two PCs it would go over the mod-to-mod data channel (`net::bus`, NETWORKING §4); remote kills between PCs are out of scope until a card asks for them.

## 4. Where it lives in the code

```
crates/nrpvp/src/console/        (behind the `dev` cargo feature, see ARCHITECTURE.md)
  mod.rs        registry: name → { help, args spec, executor, fn(args) -> Result<Cmd, Usage> }
  parse.rs      tokenizer (quotes, `;`); pure, unit-tested
  pipe.rs       named-pipe server thread; one line in, reply lines out
  target.rs     who a player command acts on
  (autoexec.rs  planned with CON-5, does not exist)
crates/nrcon/  the client
```

The executors stay where they are. The main loop already drains a command queue each tick; the console adds a reply channel so text
comes back to whoever sent it. The ImGui panel's buttons and the F-key hotkeys are deleted once their commands exist (the hotkey
list in `config.rs` shrinks by about 20 keys).

## 5. Rules

- Dev builds only (`--features dev`). The player DLL has no pipe and no parser.
- The pipe accepts the current user only and is local only. No TCP.
- Every console command is logged (`console: <line> -> <reply>`), so test logs show what was typed.
- Commands never block the main loop: they are queued and run on the next tick (50 ms).

## 6. Plan (cards)

| Card | Content | Acceptance |
|---|---|---|
| CON-1 | Command core + parser + registry, mapped onto the existing `Command`s; `help`, `echo`, `status`, `players`, `team_*`, `pvp`, `mm_*` | Unit tests; solo: `nrcon status` answers from a running game |
| CON-2 | Pipe server + `nrcon` with `-i` / `--all` | Solo in Nucleus: one command reaches both instances |
| CON-3 | New commands: `kill`, `down`, `bleed_clock`, `flag_*`, `msg_show`, settings, `pos_*` | Solo: each one, checked on screen |
| CON-4 ✅ | Delete the ImGui panel buttons and the F-key hotkeys (done with R5, 2026-10-20) | Dev build has no hotkey thread |
| CON-5 (optional) | In-game console line (ImGui) + `autoexec.cfg` | Solo |

**Agents and skills:** no new agent; CON cards fit the existing `implementer`. The skill **`add-console-command`** (`.claude/skills/`) covers registry entry, executor, help text, test and log line.

## 7. Decisions (2026-10-20)

Leonardo delegated these; Claude decided.

1. **Frontend order:** pipe + `nrcon` first (CON-1, CON-2). The in-game ImGui console line (CON-5) comes later in dev builds, for the
   CS feel when playing solo. It is the same command layer, so nothing is thrown away.
2. **`kill` and `down` are separate commands.** `down` = HP 0 (bleed-out runs); `kill` = HP 0, then the bleed-out clock to 0 once
   downed. Both use only verified writes (MEMORY_MAP §5–6) on the local player.
3. **Player build: no console at all.** It lives behind the `dev` cargo feature.
4. **Syntax: CS style, underscores in names** (Leonardo: `mm_queue_solo`, no spaces inside a name; values after a space; `;` chains).
   A leading `/` is accepted and ignored, so `/kill` works too.
5. Command list: §3 as written; new commands are added with their feature from now on.
6. **Targets** (§3.1): `me` / `p1`…`p6` / slots / `allies` / `rivals` / `all`. Owner-only writes on another player are routed
   by `nrcon` to that player's game; names come after MM-6. Targets ship in CON-1 (parser) and CON-3 (routing).

Saved presets (Arena/dev): `saved_loadouts` logs the store path and availability. `saved_loadouts sandbox` switches to a temporary
store without touching player presets. `saved_loadouts save|load <slot 1..10>` submits the character-independent ESD operation. Main-loop commands require an idle hub; native inventory work uses the game-thread queue. Results
are logged as `saved_loadouts:`.
