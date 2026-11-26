# 2026-11-25 Arena Trio (3v3) against bots, one PC, played by hand (ARENA-6)

### 2026-11-25 — Arena Trio vs bots (Leonardo, player zip v0.4.2 = commit f82354a, nrpvp.dll A4B004BEEE2A07D4). Logs: `<Game>\NightreignPvP\logs\nrpvp_2026-11-25_20-38-57.log` (copy in `.claude/experiments/2026-11-25-arena-trio-bots/`): **PARTIAL**

Game 1.03.3 (exe 1.3.3.0), SC 1.1.3, me3 0.11.0, player build (`mode=br`). Bots On, Arena card, Trio, Commence at 20:40:08.

- **PASS: entry and the first fill.** `match against bots requested ...: 3 vs 3, 5 bot(s)`; three table rounds kept enemy slots 0, 1, 2
  and ally slots 1, 2 (`5 of 5`); enemies `team 2 -> 6`, allies stay team 2; each bot `stands on its spawn`; `sides: LOCAL + 2
  teammate(s) vs 3 rival(s)` before `Round 1: fight!`. The 6th bot of the third table round (0x10E0000B) was `not needed; sent away`.
- **PASS: round 1** (3v3, 1 min 48 s): the three enemies died (two by LOCAL, one with an ally's last hit), `Round 1: you won (1-0)`,
  the allies parked and put back on their spawns with full HP at the reset, `3 bot(s) dead; new ones for the next round`.
- **FAIL: round 2 was 3v1.** The refill needed three enemies and got one: `round 4: kept bot 0x...0C (slot 1); 3 of 5; Arena enemy
  slot 0`; slot 0 of the bot manager held the bot sent away in round 1's fill again (`[0x...0B, 0x...0C]`), so it was skipped as
  already handled and not freed; `round 5: table 30000 ... armed, [0x...0B, 0xFFFF...]` then nothing until the match ended (the
  director waits for both slots, `ROUND_TIMEOUT_MS` 120 s was never reached). `Round 2: fight!` with `vs 1 rival(s)`; the ally bot
  0x...0A killed it, `Round 2: you won (2-0)`.
  Cause (from the slot reads): the clear did remove 0x...0B (`[0xFFFF..., 0xFFFF...]` at 20:40:37); at the refill the game gave
  the same handle to a new bot, and the director, which keeps every handle it has dealt with in `handled`, took it for the old
  one: not kept, not forgotten, so it blocked slot 0 and the next table round never filled both slots. Any odd count has a bot not
  needed (Trio 5, Duo 3; BR replacements too).
  **Fixed after the run (unit test only):** a bot not needed is forgotten once it is out of both slots (`BotFill::release_extras`),
  so its handle counts as new again; `a_trio_refill_keeps_a_new_bot_on_the_handle_of_a_cleared_one` replays this log (fails
  without the fix with `[ForgetSlot(1)]`). 713 unit tests, clippy dev and player clean; dev DLL in `dist\`.
- **PASS: match end.** `match over, WIN 2-0`, allies `removed (HP 0)`, `expedition over; 6 bot(s) were kept`, back at the Roundtable,
  `summary shown`. The summary is the 1v1 shape (`leo: 2 rounds won`, `Bot: 0 rounds won`), no team rows.
- **Reset moves:** at both resets LOCAL's spawn move was refused for about 1 s (`Physics+0x50 does not read as a heading`, ~20 WARN
  lines each) and then went through (`moved 15.8 m`, `moved 32.2 m`) before the fight started (the `PLACE_WAIT_MS` wait).
- **Not covered:** the team spectate (LOCAL was never beaten), the switch inputs, a loss.
- Log wording only: a bot killed by LOCAL is logged `last attacker 0x...10C00000 = Rival` (LOCAL, from the bot's side) and the
  scoreboard says `Bot n is gone (a bot replaced after a mob's kill)` for every dead Arena bot.
- No ERROR or panic. Heartbeat slowest tick 40-180 ms (same range as 2026-11-20).

### 2026-11-25 (re-run) — Arena Trio vs bots with the refill fix (Leonardo, dev DLL from branch `ultrawide` = dca2c1f, contains 423ffdb). Log: `dist\nrpvp.log`, session started 23:22:47 UTC: **PASS**

Game 1.03.3 (exe 1.3.3.0), SC 1.1.3, me3 0.11.0, `scripts\launch.ps1` (profile sc), developer build. Bots On, Arena card, Trio,
Commence at 23:23:53. `ultrawide: on (exe+0x1B8B286 74 -> EB)` at startup (UW-1 on by default).

- **PASS: first fill** as on 2026-11-25: enemy slots 0, 1, 2 and ally slots 1, 2 (`5 of 5`), the 6th bot (0x...0B) `not needed; sent
  away`, `sides: LOCAL + 2 teammate(s) vs 3 rival(s)`, `Round 1: fight!` 23:24:41.
- **PASS: the refill (the bug of the first run).** Round 1 won 1-0 (23:26:25), `3 bot(s) dead; new ones`, then `round 4: kept bot
  0x...0B (slot 0); 3 of 5; Arena enemy slot 0`: the handle of the bot sent away in the first fill came back on a new bot and was
  kept this time; slots 1 and 2 filled too (`5 of 5`), the extra bot 0x...11 sent away, `vs 3 rival(s)`, `Round 2: fight!` 23:26:31.
- **PASS: match end.** `Round 2: you won (2-0)`, `match over, WIN 2-0`, `expedition over; 8 bot(s) were kept`. The game was closed
  before the Roundtable, so `summary shown` was not reached (the summary of 7 rows was waiting).
- No ERROR or panic; 19 WARN = LOCAL's spawn move refused for about 1 s at the resets (known, then moved).
- **Not covered:** the team spectate against bots (LOCAL was never beaten), a loss, a Duo match against bots. No killfeed line for a
  bot's death: by design in the Arena (`fill.rs` ARENA-4 filter; the round toasts say who won).

### 2026-11-25 (second re-run) — Arena Trio vs bots, killfeed lines (ARENA-KF) (Leonardo, dev DLL from branch `ultrawide` = b7f027b). Log: `dist\nrpvp.log`, session started 23:38:13 UTC: **PASS** (killfeed), round 2 not played

Same setup as the first re-run (`scripts\launch.ps1`, developer build, Bots On, Arena card, Trio; Commence 23:59:15).
- Fill as before: enemy slots 0, 1, 2, ally slots 1, 2, the extra 0x...05 sent away, `Round 1: fight!` 23:59:44.
- **PASS: killfeed lines for bot deaths in an Arena Trio.** Each enemy's death gave a `killfeed: bot event` and a toast:
  `leo killed Bot 1` (00:00:41), `leo killed Bot 3` (00:01:12), `leo killed Bot 2` (00:01:34), LOCAL in blue (#7FC8FF), the bots
  in red (#FF6A5A). The last kill's line showed 0.5 s after `Round 1: you won (1-0)` (two toasts, no overlap reported).
- The refill started round 2 (`round 4: kept bot 0x...05 (slot 0)`: the handle sent away in the first fill, kept again; 0x...09 slot
  1); at 00:01:41, during the second table round, the world unloaded and the log ends at 00:01:43 with no ERROR or panic: the game
  was closed (summary `not finished (1-0)`). Leonardo: "everything worked fine".
- **Not seen:** a bot ally's kill and an enemy bot's kill of an ally (all three kills were LOCAL's; unit test
  `a_duo_bot_allys_kill_gets_a_killfeed_line`), a line for a bot on a reused handle (unit test).
