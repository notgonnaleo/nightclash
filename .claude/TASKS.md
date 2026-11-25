# Task Board

The unit of work is the **task card**. Opus writes and reviews cards. Sonnet executes one card at a time.
The Human runs anything that needs the game open.

**Status values:** `todo` · `ready` (all dependencies done) · `in-progress` · `review` (Sonnet done, waiting for Opus) ·
`done` · `blocked` (with reason).
**Owner values:** `SONNET/implementer` · `SONNET/researcher` · `OPUS` · `HUMAN` · combinations, where the
first owner does the work and the second verifies.

**Testing constraint:** day to day, only Leonardo tests, solo. Two-player checks are batched into
**Friend Sessions** (FS-1, FS-2, …). Their runbooks are in TESTING.md §6. A card is `done` when its
**solo** acceptance passes. Its multiplayer checks then sit in the FS runbook.

## How to run a card

- **From an Opus session:** "Run T-00X." Opus spawns the matching agent from `.claude/agents/` with the card
  text, reviews the result, and updates this board. Several agents may run in parallel, so **agents do not
  edit this file**. They write their Result into their deliverable or reply, and Opus copies it here.
- **From a Sonnet session directly:** "Execute T-00X from .claude/TASKS.md, following .claude/CLAUDE.md." When
  it's done, it sets the status to `review` and fills in Result. Opus moves the card to `done`.
- **HUMAN steps:** follow the card. Paste logs and observations into chat or into TESTING.md, and an agent tidies them up.

## Card template

```
### T-XXX — <title>
Owner: … | Status: … | Depends on: … | Phase: …
Goal: one sentence, the question answered or the capability added.
Context: links to the doc sections the executor must read.
Steps: numbered and concrete.
Deliverables: files created or changed.
Acceptance: objective checks; mark the HUMAN-solo checks; two-player checks go to an FS runbook.
Out of scope: what NOT to do.
Result: (filled in after execution: what happened, evidence, follow-ups)
```

---

**Board cleaned 2026-10-20:** done, dropped and superseded cards were removed. Their full text is in git:
`git show 339ee04:.claude/TASKS.md`. Findings from them live in `re/MEMORY_MAP.md`, `MATCHMAKING.md`, `TESTING.md` §3 and
`archive/research/`.

---

## After the friend session of 2026-11-07 (Leonardo's list; first priority; evidence in testing-log 2026-11-07 "Friend session FS-HM1 (part)")

Leonardo (2026-11-07): the mod is close to done; nothing broke the game; the list below is cosmetic or odd bugs. Duo / Trio still untested
with friends. Suggested order: FS7-1 (two of the seven points), FS7-2, FS7-3, FS7-4 (small, one PR together is fine), FS7-6 step 1 (a log
line for the next session), FS7-7, then FS7-5 (needs RE). Owner of all: OPUS cards → SONNET/implementer → OPUS review; acceptance per card.

| Card | What | State | Size |
|---|---|---|---|
| FS7-1 | Spectator hint and own K/D only on the own screen (also fixes the previous match's K/D) | **Built, `mi-msg-local` PASS** (2026-11-08) | Small |
| FS7-2 | Battle Royale card first in the expedition list | **Built, `mi-bots-menu` + `mi-zero-click` PASS** | Tiny |
| FS7-3 | Bots switch Off at every game start | **Built, `mi-bots-menu` PASS** (also re-set when the settings tab opens) | Small |
| FS7-4 | Solo / Duo / Trio preselected from the party size | **Built**; next FS (a party at the table) | Small |
| FS7-5 | Respawn on the hawk after a mob death | **Built, `mi-respawn-hawk` PASS** (2026-11-08; 150 m up, 15 s steerable ride) | Large |
| FS7-6 | Solo instant death at 0 HP (gauge: decided, stays hidden) | **Fixed** (branch `fs7-6-die-flag`): our test flag 10007420 left ON in Leonardo's save, now cleared at every expedition start | Small |
| FS7-7 | Summary ranking after a trade | **Built** (the real cause: shifting team keys), unit test | Small |
| FS7-8 | F10 also leaves the Seamless Co-op lobby (the game session already does) | **Built** (Solo only); next FS | Small |

**Built 2026-11-08 on branch `fs7-small-fixes` (one PR):** FS7-1 `scripts/emevd_add_events.py` puts `Set Network Sync State Disabled`
first in every display event, and the own-score text is reset to zeros when an expedition ends. FS7-2 `sortId` 1 (smallest vanilla 10);
`nro` no longer presses `Up` to reach the card. FS7-3 / FS7-4 `menuflow::preset_menu` when the expedition menu opens (the queue idle):
Bots Off at the first menu of a launch; Number of Players from `mm::party_size`, which the worker publishes from the Seamless Co-op
lobby (in the last match's lobby: the match's own team size, so Solo rivals are no party). FS7-7: the summary groups teams by pairs seen
with the same key **in the same tick** (`Feed::mates`); the key is a roster index that shifts when a player leaves, which merged a
leaver with a survivor in both summaries of 2026-11-07. Left open: the decision and the summary can still disagree after a trade
(elimination waits `OUT_DEAD_MS` for a rival's death, the summary uses the kill facts). FS7-8: `Act::LeaveSession` on F10 / the
defeat-exit timer; back at the Roundtable the worker leaves the Seamless Co-op lobby (`QueueSteam::leave_sc`), **only in Solo** (a
Duo / Trio lobby also holds the own party). Unit tests, clippy (both builds), gamedata and DLL build pass; one-PC checks PASS 2026-11-08 (testing-log); FS7-4 / FS7-7 / FS7-8 wait for the next FS.

### FS7-1 — Our messages only on the own screen (spectator hint, own K/D) + stale K/D
Status: ready. Points 1 and 6 of Leonardo's list.
Cause `[HYPOTHESIS, from code]`: the display events built by `scripts/emevd_add_events.py` (`display` = `blinking` / `dialog` / `status`, e.g.
99100806 flag 10007413 spectator hint, 99100807 flag 10007414 own score, 99100310 flag 10007110 summary) do **not** start with
`Set Network Sync State Disabled` (2000:2), unlike our per-game events (`local_instrs`, `speffect_instrs`). So one game's display is shown
on every game, and each game shows **its own** text entry: the receiving game's 99100601 still holds what it wrote last, often in the
previous match (point 6). The own tally itself resets correctly (log).
Steps: (1) add 2000:2 first in every display event (`build_event_instrs`), rebuild gamedata; (2) when an expedition starts, rewrite 99100601
with `Kills: 0 | Deaths: 0` (defensive); (3) if (1) does not stop it, fall back to the toast path (`gametoast`, local by construction).
Acceptance: new scenario `mi-msg-local` (two instances in one expedition, like `mi-killfeed`): P1 `flag 10007414 1` and `flag 10007413 1`
→ screenshot of both; P1 shows the message, P2 does not; `score-bots`, `spectate`, `score-bots-end` still PASS. Next FS: nobody sees
another player's spectator hint or score.

### FS7-2 — Battle Royale card first in the expedition list
Status: ready. `gamedata/nightbossmenu-rows.json` row 9 `sortId` 310 (after The Deep of Night, 300) → below the smallest vanilla `sortId`
(read every row first with `sbwriter dump`). The card is recognised by the chosen target list, not by its place (MEMORY_MAP §13), so
nothing else should change. Acceptance: one-PC screenshot of the list with the card first (extend the card lab or `mi-bots-menu`), and
`mi-zero-click` + `mi-bots-menu` PASS. Check which card the cursor starts on.

### FS7-3 — Bots switch Off at every game start
Status: ready. The game saves the "No. of Password Players" byte (`MENU_SETTINGS_BOTS`, layout.rs), so Bots: On survives a restart. The
first time `menuflow` sees the expedition menu after a launch, write Off (`settings.set_bots(false)`) and log it; later changes in the
same launch stay. Acceptance: scenario `bots-off-at-start` (`bots_switch on`, restart the game, open the menu → log `Bots switch reset to
Off`, `bots_switch` reads Off).

### FS7-4 — Solo / Duo / Trio preselected from the party size
Status: ready. When the expedition menu opens, set the Number of Players byte (`MENU_SETTINGS_PLAYERS`: 2 Solo, 1 Duo, 0 Trio) from the
Seamless Co-op party at the Roundtable (members of the SC lobby we are in, the count `mm_queue` already reads; no lobby = Solo); a
choice the player makes in that visit is kept. Acceptance: `mi-duo-party` gains `br_card` → `players byte 1` on both after the invite and
before any press; a one-game run reads `players byte 2`.

### FS7-5 — Respawn on the hawk after a mob death
Status: **built 2026-11-08 (branch `fs7-5-hawk-respawn`), `mi-respawn-hawk` PASS**; next FS: how it feels with real players. Leonardo chose the
Warzone-Resurgence model (a short steerable redeploy from the air, not the match-start crossing: the start hawk cannot be started again,
TECHNICAL_NOTES §4b). Every mob-death respawn (RW-2; RW-7 still picks the place far from rivals) puts LOCAL 150 m above it on a
tree-hawk ride (`Act::HawkRespawn`, `hawk::request_ride`); the player steers and leaves with F / Alt; after 15 s the mod drops a
player still riding (377 = 2, vanilla separation fall, no damage). Open: a ride near the map edge can fly out over the void before
the 15 s drop (not seen; check in the FS), the tunable values are constants (`RESPAWN_LIFT`, `RESPAWN_RIDE_SECS`).

### FS7-6 — Solo: instant death at 0 HP
Status: **built 2026-11-08 (branch `fs7-6-die-flag`), `mi-respawn-hawk` shows the reset**; Leonardo's save is fixed by his first match on the new build. Not a game rule and not host / client: **Leonardo's own save had our
die-at-once flag 10007420 ON**, so his character carried SpEffect 102130 (die at once, no downed state, TECHNICAL_NOTES §4) in the
friend session. Evidence: fuso, a client in M1, went downed on all 5 of his mob deaths (`T-040 LOCAL state 0 -> 1`); Leonardo's log
has no downed state at all; the dev scenarios `defeat-mob`, `defeat-rival`, `score-bots` run on Leonardo's real Steam account and set
`flag 10007420 1` (last 2026-11-07 16:37 UTC, `set ON (was ON)`), and our flag group is saved (TRAPS). The mod's own reset when an
expedition ends never runs: it reads `world.night`, which is only filled inside an expedition (`World::after_combat`), so it is
always `None` at that tick. Fix: clear 10007420 when an expedition starts (the player build never sets it; scenarios set it after
entering), log it, golden test; remove the dead reset. Acceptance: unit/golden test + `defeat-mob` still PASS. The downed rival's
gauge: decided, stays hidden (DECISIONS 2026-11-07). Open question for Leonardo, no change: `rival_hit_seconds` 3 s per hit on a
downed player (4 quick hits end a 15 s bleed-out).

### FS7-7 — The end after a trade (both kill each other)
Status: ready, small. M2 ended with Leonardo and Samuka killing each other 2 s apart: `match decided ... (this game's player lost)`, but
the summary ranked Leonardo `1.`, fuso `1.`, Samuka `2.`. Write the rule (SCOREBOARD §3: last to fall places higher; a true tie = shared
place) and a golden timeline for the trade; the summary order must agree with the decision.

### FS7-8 — F10 while defeated also leaves the session
Status: ready (DECISIONS 2026-11-07). Evidence (fuso, host in M2): F10 at 19:35:15 left the **game session** 10 s later (`role host ->
other -> none`) and the others migrated at once, but fuso **stayed in the Seamless Co-op lobby**: Leonardo's `mm_join: gate` lines list all
3 `sc_members` from 19:35 to 19:37:44, until fuso quit the game (his `SC lobby ... gone` at 19:38:12). That is the "still in the
session at the Roundtable" Leonardo saw. The game session part already works (the migration follows at once). Missing: after
the F10 exit reaches the Roundtable, also leave the Seamless Co-op lobby the way `hm_leave_lobbies` does in `mi-host-migration` (Steam
`LeaveLobby` of the SC lobby), so the leaver is out of the party and can queue again. Check first on two instances that the leaver's own game stays sound
(no title screen, Commence works). Acceptance: scenario `mi-f10-leave` (three instances like `mi-host-migration`: P1 host defeated by a
rival → F10 → P1 `role ... -> none` and back at the Roundtable; P2 / P3 `role client -> host` / `client -> client` with the new id and
still in the expedition; P1 queues again). Clients pressing F10 leave the same way.

Logs of that session: Leonardo's and fuso's are in `.claude/experiments/FS-2026-11-07/`; Samuka's are missing (not needed for any card).

---

## PvP rules

**Done 2026-10-22: T-040 and T-041 accepted** (Leonardo: "worked very well"; TESTING §3). A rival's hit on a downed player is now
cancelled inside the game (hardware breakpoint on the bar store `exe+0x66DFC4`): no stand-up, no gauge flicker, the bleed-out penalty
stays; teammates revive normally (3-instance run, `team_size = 2`). Fallbacks still on: the revive-event block (`exe+0x675159`) and the
T-040 re-down (data-module HP, 2 s retry). Cards removed; text in git (`git show 3384bca:.claude/TASKS.md`); full story and addresses in
`archive/tasks/T-041-no-getup-handoff.md` and MEMORY_MAP §6. Follow-ups (no card yet): a player zip with this build for the next FS (real
two-PC lag); confirm in the player build that the hooks' two hardware slots (0/1, only while downed) never clash with the quiet invite (0/1, only at
the Roundtable at match ready).

---

## Engineering (accepted 2026-10-20; cards are written out in full when they start)

Done (on `main`, SMOKE-1 PASS 2026-10-20): **R0** (fmt + CI), **R1** first pass (dev feature), **CON-1, CON-2** (console + `nrcon.exe`).
Done on `main` (SMOKE-2 PASS 2026-10-21): **R5**, **CON-4**, **R2** half (`game/layout.rs`), **R3** half
(split by job), **R4** removed keys. Open: the R2/R3 halves (with the first new feature), R4 sections, R6, CON-3, CON-5. Order was: SMOKE-1 → FS-UI2 → R5 → R2 → R3 → R4 → R6 → CON-3, CON-4 → (CON-5 optional).
- **R0–R6 — Refactor plan.** Behaviour-preserving; scope, acceptance and rules in `ARCHITECTURE.md` §6–§7. Owner: SONNET/implementer →
  OPUS review → HUMAN solo smoke run.
- **CON-1–CON-5 — Developer console.** Scope and acceptance in `CONSOLE.md` §6–§7. Owner: SONNET/implementer → HUMAN solo.
- **VER-1 — Version gate** (refined in the 2026-10-24 review). `startup.rs` only logs the exe FileVersion. Build a pure `version::gate(file_version, pinned) -> Open | Closed(reason)` in `startup::run`; `Closed` (also when unreadable): one log warning and **no** `hwbp::set`, VEH, memory write, event flag, `poswrite` or teleport; the Steam worker and read-only `World` may run. `deathhold` and `reviveblock` already byte-check their RVAs; `menuflow`, `quietinvite`, `hawk` and the flag writes do not, so the gate is what protects a patched exe. Acceptance: unit tests, a dev override env `NRPVP_FAKE_EXE_VERSION` and a scenario `version-mismatch` (gate line logged, no `hwbp:` lines). Owner: SONNET/implementer. Rule source: ARCHITECTURE §6 "Modding rules".
- **HWBP-1 — Claim/release API for the debug registers** (2026-10-24 review; ties into the decoupling refactor). `hwbp::set` silently overwrites a slot's holder, `is_free` then `set` is not atomic, and features mix fixed slots (quiet invite 0/1, Commence 2, labels 3) with "first free" (deathhold, reviveblock, puppets, itemdrop). Build `hwbp::claim(owner, addr) -> Result<Slot, Busy>` (CAS per slot, owner beside the address), `release(Slot)` by the owner only, `holders()` for the log and `status`; a refused claim logs the holder. Write the peak budget table into ARCHITECTURE (player build worst case: menuflow 2 + reviveblock 2 = 4, so a death hold during a downed state has no slot). Acceptance: unit tests (double claim, release by a non-owner, budget exhausted), `defeat-rival` and `match-loss` PASS, `hwbp:` log shows owners. Owner: OPUS design, SONNET/implementer.

## Launcher

### LAUNCH-GATE — The launcher refuses an old mod
Owner: OPUS (built) → HUMAN (acceptance) | Status: review | Depends on: — | Branch: `launcher-version-gate`
Goal: `nrpvp_launcher.exe` and the launcher app's Play do not start the game when the installed mod is older than the latest
published release, and tell the player how to update. Rules and behaviour: RELEASE.md §5c; no check (offline) = no start either
(DECISIONS 2026-11-17, Leonardo).
Built: `crates/nrpvp-launcher/src/version.rs` (installed version from `VERSION.txt` / `release.json`, `decide`, message),
`release.rs` moved there from the app (one lookup for both), `launch()` runs the gate and returns `LaunchError::Outdated`; the
launcher exe's box offers the download page; the app's dock shows "Update required" and has no "Play without updating" any more;
`package-release.ps1 -Version` writes `version X.Y.Z` (publish passes it), without it `version local`. Unit tests: comparison,
`VERSION.txt` parsing, latest / outdated / unknown / local decisions.
Acceptance (HUMAN-solo, game installed with a zip that has the gate; needs internet; ~10 min, no match needed):
1. Note the latest release on https://github.com/notgonnaleo/nightclash/releases/latest (vB).
2. Edit `<Game>\NightreignPvP\VERSION.txt`: first line `version 0.0.1`. Double-click `nrpvp_launcher.exe` → a box "Your Nightreign
   PvP is v0.0.1, but vB is out" with Yes/No, the game does **not** start; `NightreignPvP\launcher.log` = `version gate: BLOCKED`.
   Yes opens the releases page.
3. Same file, open the launcher app (`NightreignPvP-Launcher.exe` with `--skip-auto-update`, so it does not update by itself) →
   the bottom bar says "Update required: vB" with Update; no Play. Run it without the flag → it updates and shows Ready.
4. First line `version vB` (the real one) → `nrpvp_launcher.exe` starts the game; `launcher.log` starts with `version gate: OK`.
   Same from the app's Play.
5. Real version, network cable out / Wi-Fi off → `nrpvp_launcher.exe` says it could not check and the game does **not** start;
   `launcher.log` = `version gate: BLOCKED, latest unknown (...)`. The app shows "Could not check for updates" with Retry; network
   back, Retry → Play.
Out of scope: blocking a mismatched Seamless Co-op or game exe (the app only warns); a cached latest version.

## Test tooling

The orchestrator (`nro`, scenarios, puppet players) is in this repo and is the primary test route: `TOOLS.md`, `orchestrator/README.md`.
ORCH-1 (virtual pad) is blocked and parked (the game scans XInput pads only on a device change; keyboard and mouse through `nro desk`
work). Open ideas: `puppet_revive`, puppets that attack, scenarios that wait on the log file instead of the 300-line console tail.
Every card whose acceptance can be a scenario names it (`scenarios/<name>.toml`) and is done when `scripts\regress.ps1 <name>` is PASS.

### NRO-MI — `nro` launches 2-3 real instances (Nucleus replacement)
Owner: OPUS (plan) → SONNET/implementer | Status: S0-S3 done; `mi-join` PASS 3/3 (2026-10-27); open: `mi-duo-sc` (team_size 2 teammate run), never written | Depends on: none
Goal: Claude drives and sees every player's view (host/client, Duo/Trio) with no human and no Nucleus.
Context: full design, spikes S0-S4, risks: [orchestrator/MULTI-INSTANCE.md](orchestrator/MULTI-INSTANCE.md).
Acceptance: `scenarios/mi-join.toml` (PASS) then `mi-duo-sc.toml` PASS (both consoles `players` = 2) + a `testing-log/` entry.
Out of scope: real Steam lobbies, real lag, shipping an emulator.

## Matchmaking and in-game UI (M8)

**Done 2026-10-21 (FS-UI2 PASS, TESTING §3):** **MM-4** (Solo queue, player build), **UI-1** (Battle Royale in the table menu, steps
1–5) and **UI-2** (status as blinking game messages). Cards removed; text in git (`git show c727293:.claude/TASKS.md`). The menu and
text addresses they found are in `re/MEMORY_MAP.md` §13; the flow is in `MATCHMAKING.md` §3. Left over as polish (not cards yet): the
live count inside the searching box (UI-1 C3), and messages that never show because the next state arrives first (TESTING §3, FS-UI2 entry).

### MODE-SCOPE — Mod rules only in the Battle Royale expedition (Leonardo, 2026-11-02, not urgent)
Owner: OPUS (design) | Status: idea, not scheduled
The normal expeditions started from the other cards are vanilla in every checked way, except that the hawk crossing at the start (and maybe other rules: free Day 1, PvP team writes, HUD filter) also runs in them although it is meant for Battle Royale. Decide which
rules follow "this expedition came from our card" and which stay always on; the card signal exists (`menuflow::decide_for_card`), the expedition itself would need a flag set at Commence. First check what really changes in a normal expedition (one short run).

### MM-6 — Duo and Squad with rivals: Steam ID → expedition player
Owner: OPUS to card in detail | Status: **started 2026-11-02**: the menu queues Duo / Trio (`br_menu`), a Duo party of 2 reaches an expedition as teammates (`mi-duo-party`); open: the 6-player 3-team match, Trio, messages for a party member / no party, the roster → team mapping with several teams | Settles: S9 U7
Match sizes (Leonardo, 2026-10-19): Duo = 3 teams of 2, Squad = 2 teams of 3, always 6 players. ~~Complete parties only~~: since
2026-11-08 a short party is filled with random players (MM-FILL, DECISIONS).
With more than one team in a Duo/Squad match the roster (Steam IDs) must be mapped to expedition players on every client.

**Direction 2026-10-21:** no Steam window on any screen, everyone arrives ready and the match starts itself, the host is invisible and chosen by the matchmaker; host leaving = host migration (HOST_MIGRATION.md).

**Done 2026-11-05: MM-3g and MM-7** (`mi-zero-click` PASS 3/3; `game/matchstart.rs`, MEMORY_MAP §12). Their real-Steam check is FS-HM1 item 9. Left as polish, no card: the table menu stays open during the departure. Cards removed; text in git.

### MM-FILL — Duo / Trio fill a short party with random players (before the first release)
Owner: OPUS (spec, review) → SONNET/implementer → FS | Status: **built 2026-11-08 (branch `mm-fill`, parts A+B+C), `mi-duo-fill`
PASS; left: the FS checks below** | Depends on: MM-6 roster → team mapping (part B below, which premade Duo / Trio with more than one team need anyway) | Phase: before release (Leonardo 2026-11-08)
**Built:** A `mm/pack.rs` + `fit` / `bounce` keys; B `nrpvp_pid` on the SC lobby → `mm::roster_teams` → `build_view` (partial map =
unknown players neutral; duplicate ids, i.e. Roundtable ids, = no map); C a lone Duo / Trio player leaves the match lobby back at the
Roundtable. Not seen yet: 3 teams, Trio, a bounce on real Steam, part C (needs a finished match), a premade pair against a filled pair.
Goal: a player alone (or a party of 2 in Trio) who picks Duo / Trio and presses Commence is queued and gets random teammates, as in
any battle royale; today `build_party` (`mm/machine.rs`) refuses with "Duo needs a Seamless Co-op party".
Context: MATCHMAKING.md §3–§4; `mm/machine.rs` (`build_party`, ticket report, MATCH READY roster), `mm/proto.rs` (`RosterEntry`,
tickets), `rules/teams.rs`, `game/combat_state` (`team_key`), re/MEMORY_MAP.md §12 and line "Each player's Steam ID inside the game" (U7, open).
**Why it is not only a queue change** `[VERIFIED: code 2026-11-08]`: in the match the teams are **not** taken from the queue roster.
Every game sorts the session player ids (`ChrIns+0x78`) and chunks them by `team_size` (`rules/teams.rs::assign_teams`). With one team
(`mi-duo-party`, 2 players) that is right by luck; with 3 teams of 2 a premade pair or a filled pair can be split. So the fill needs
the roster's team to reach the match (part B).
Steps:
- **A. Queue packing (small, pure logic + FakeSteam).** Allow a short party (Duo: size 1; Trio: size 1 or 2; Solo unchanged). The queue
  owner packs tickets into teams of `team_size` (a party is never split; largest parties first, ties by Steam id) and writes a team
  number per member in `roster` (`id:party:team`, bump `proto`). A lobby only takes a ticket it can still pack: the owner publishes the
  biggest party it can still fit (e.g. `fit`), searchers filter `fit >= my size` (numerical filter; check FakeSteam and
  `steam_api64.dll` have `AddRequestLobbyListNumericalFilter`). Trio is the only mode where packing can fail (2+2+2); Duo always packs.
  The Seamless Co-op party lobby of a short party still travels as one ticket (the leader queues, as today).
- **B. Roster team → match team (medium, no RE planned).** Each game writes its own session id (slot 0 = local, MEMORY_MAP) and its
  roster team number as **member data on the Seamless Co-op lobby** (`nrpvp_pid`, `nrpvp_team`) once the expedition loads; every game
  builds `id → team` from the members and uses it for `team_no` instead of the chunking; chunking stays the fallback when a member has
  not written it. `[HYPOTHESIS]` our member data on SC's lobby is harmless to SC and the session id is stable for the whole
  expedition: check both first (one log line each, `mi-duo-party`). Host migration must keep the mapping (HOST_MIGRATION.md).
  Fallback if member data on SC's lobby is refused: RE U7 (Steam id of a ChrSet player).
- **C. After the match.** Random teammates leave each other's Seamless Co-op lobby as Solo players do (FS7-8 `leave_sc`); a premade
  party member stays with the own party. Status lines for "Searching for teammates...".
Deliverables: `mm/machine.rs`, `mm/proto.rs`, the team source in `game/combat_state` / `rules/teams.rs`, unit tests, a scenario,
MATCHMAKING.md §3–§4 (Duo / Trio flow, new keys), CONSOLE.md if a command changes.
Acceptance: unit tests for packing (Duo 1+1+2+2, Trio 2+1+3, Trio refuses a third 2) and for `id → team` with shifted ids;
`mi-duo-fill` (NRO-MI, two single instances pick Duo, `mm_max_players = 2`) reaches the expedition as teammates with `team from roster`
in both logs; `mi-duo-party` still PASS. **FS (needs 4+ players):** 2 teams of 2 where one team is filled, and a premade pair against
a filled pair; teammates are allies, rivals are hostile, teams match the roster on every game.
Out of scope: skill-based matching, choosing teammates, the 6-player size in the player zip (still the pre-release flag), bots as teammates.
Result: —

### MM-8 — Host chosen by the matchmaker (best connection)
Owner: OPUS (spec) → SONNET/implementer → FS | Status: idea only (matchmaking declared finished, DECISIONS 2026-11-07) | Depends: —
Today the queue lobby owner becomes host (luck). Each member publishes its Steam ping location (`ISteamNetworkingUtils`
`GetLocalPingLocation` → member data); the owner picks the member with the lowest worst-case estimated ping to the others
(`EstimatePingTimeBetweenTwoLocations`), ties by Steam id, and writes `host`. FakeSteam tests for the pick; the player never sees who
hosts. Verify the Steam API calls exist in the game's `steam_api64.dll` first.

## Host migration and 12 players (top priority: migration first, player count later; Leonardo 2026-11-02; plan and budgets in [HOST_MIGRATION.md](HOST_MIGRATION.md))

**HM-1 done 2026-11-07:** host migration works on real Steam with 3 players (two migrations in one match, the match went on; testing-log
2026-11-07, Leonardo confirmed). Card removed. Left for a later session (no card): Day 2 started by a migrated host (FS-HM1 item 4) and a
defeat drop made by a migrated host (item 5). Next on this line: player count, when Leonardo asks.

## Own networking (NET; decided 2026-10-30, design and gates in [NETWORKING.md](NETWORKING.md))

Order: NET-1 and NET-2 are done (2026-10-30; scenarios `mi-net-ping`, `mi-net-join`, `mi-net-verdict`; facts on `net::bus`, NETWORKING §4); users today: RW-3 `ITEM_DROP` (topic 5, on) and the RW-5 skeleton (`ORB_CLAIM`, topic 2, off). NET-5 round 1 is done (NETWORKING §2b) and continues as NET-7; NET-3..5 are read-only studies that feed gate G1 (NET-6: replace SC's session layer, yes or no). Hard rule 3 holds for all of them (observe `nrsc.dll` at run time only).

### NET-3 — Study: the game's own P2P traffic (read-only)
Owner: SONNET/researcher + OPUS review | Status: todo | Depends on: none (better after NET-1)
Goal: packet type / size / rate histogram of the game's `SendP2PPacket`/`ReadP2PPacket` (SteamNetworking006) and of SC's Messages channels, per phase (Roundtable, loading, expedition, a rival hit).
Steps: hardware-breakpoint trace like `steam/lobtrace.rs` on the interface's send/read vtable targets (dev build only), log `(peer, size, channel, first byte)` aggregates; `ISteamNetworkingMessages` send/receive likewise to learn SC's channel numbers. Two instances, `mi-join` flow.
Acceptance: `scenarios/mi-net-trace.toml` produces logs with non-zero histograms for each phase; findings table in NETWORKING §2/MEMORY_MAP (type ids, which carries hits/HP/SpEffects); UNKNOWN 2 and the channel question answered or restated.
Out of scope: injecting packets, changing any game or SC byte.

### NET-4 — Study: NR `CSSessionManager` layout (read-only)
Owner: SONNET/researcher + OPUS review | Status: todo | Depends on: none
Goal: map the singleton at `exe+0x3C21EC8` (state, `players` vector, steam id offset, send queue, limit fields) and `CSTeamSessionManager`.
Steps: dump the object in a 2-instance session (`memscan`/`peek`), match each instance's Steam id (user getter), cross-check with ORCH-R2 (`[[exe+0x3C078D0]+0x10]+idx*0x8A0`) and the ER layout; answer whether `0x6001` is the engine's or SC's numbering.
Acceptance: table in `re/MEMORY_MAP.md` with a `[VERIFIED]` tag per offset; console `session` dump command; scenario `mi-session-dump` logs both players with their Steam ids.
Out of scope: writes.

### NET-7 — Sign-in gate: make the game "signed in" without Bandai's server (the decider for replacing SC)
Owner: OPUS + SONNET/researcher | Status: **released with guards for the lab stage (DECISIONS 2026-11-02), runs as HM-2 only if HM-1 shows SC blocks migration**; real Steam needs a separate go | Depends on: NET-5 round 1 | Budget: 3 to 5 working days, then stop and decide (NET-6)
Goal: find what takes the three sign-in state objects from 3 (request in flight) to 2 (done), and complete it ourselves with local data, so that Invite Members works in a game without `nrsc.dll`.
Steps: (1) in an SC run (the oracle) find the writer of state 2 and the function that handles the sign-in answer: reset the state, run `rawcall F18D30 0`, trace writes with hardware breakpoints on the state words and the step function `exe+0xDEC110` (static: its callee `0xDEBA10`, the request builder, and the 0x41-byte profile at `0xE01370`); (2) call that completion from our mod after our own conditions (never with the real server reachable: `Menu.IsEnableOnlineMode` stays false, flag byte handling decided by what (1) shows); (3) set `CSSessionManager+0x170` to 6 and look at what the table menu and the lobby do; (4) try `mi-nosc-join` again.
Acceptance: `mi-nosc-join` PASS: Invite Members creates a lobby (`LobbyCreated_t result=1`) and instance 2 joins it, both consoles show `world loaded, 2 player(s)`. **Stop rule:** if after 5 days the sign-in cannot be completed locally, or the next gate is a server call that needs a real answer, record the blocker and take NET-6 with "no": the data channel (NET-1, NET-2) stays the plan.
Out of scope: widening the member table beyond 2 remotes (only after this passes), real Steam, any connection to Bandai servers.

### NET-6 — Gate G1: replace the session layer, yes or no
Owner: OPUS → Leonardo decides | Status: blocked on NET-3..5
Goal: a decision in DECISIONS.md with evidence, size estimate, the read EULA/support text (UNKNOWN 8) and, if still wanted, the question to SC's author.
Out of scope: starting S1.

## Offline mode (decided 2026-10-22; first new feature, so it carries the open R2/R3 halves)

Leonardo's decision: a separate **Offline** mode for solo play and solo testing, so `br_menu` / `matchmaking` never have to be switched
off again. The solo test mode (no queue, no second player) is done; rivals for tests are the orchestrator's puppets.

**OFF-1 done (2026-10-22, solo PASS, TESTING §3):** `mode = "br" | "offline"` + console `mode`; Offline lets Commence Solo run the
game's own solo expedition. Card removed; text in git (`git show 4e0180c:.claude/TASKS.md`).

### OFF-2 — Offline mode in the game menu (players)
Owner: OPUS (RE) → SONNET → HUMAN | Status: later (after OFF-1)
A player-visible way to pick Offline in the stock expedition menu (e.g. an extra Number of Players entry or a Roundtable message
choice, UI-1/UI-2 tools). Design after OFF-1 is in use.
Sketch (2026-10-22, not designed): the Number of Players list already shows our Solo / Duo / Trio through the label hook
(`menuflow`, hwbp slot 3). The cheapest entry is a fourth choice there is none of: so either (a) reuse a value: Commence pressed
twice within a few seconds with Solo = Offline (no new UI, but hidden), or (b) a Roundtable event (UI-2 tools) that asks "Battle
Royale or Offline?" when the player first walks to the table and sets `mode` through a flag the DLL reads (flag group 10007).
(b) is the player-friendly one; it needs a yes/no dialog event (2007 family) whose answer lands in a flag: check that first.

**Bots were cancelled on 2026-10-24 and are back as a feature on 2026-11-06:** see the "Bots" section and [BOTS.md](BOTS.md).

## Scoreboard (Leonardo, 2026-11-07: first priority; design, rules and open questions in [SCOREBOARD.md](SCOREBOARD.md))

### SB — K/D scoreboard: own numbers in the match, everyone's at the end
Owner: OPUS designs and reviews, SONNET/implementer builds one piece at a time | Status: **SB-4 built (end-of-match summary, `score-bots-end` PASS 2026-11-07); SB-1 built and the own score built
as the game's centre message (Leonardo's choice, not a toast); `score-bots` PASS, Leonardo saw it over the open menu; next: the end-of-match summary (SB-4)** | Depends on: KF-1, BOT-KF (on `main`) | Phase: player feature
Goal: during a match each player sees only their own kills and deaths as they change; when the match ends every game shows every
participant's kills and deaths (players and bots).
Context: SCOREBOARD.md §2-§7 (rules, data path, UI options); re/TOAST_KILLFEED.md (toasts, `Feed`); BOTS.md §2d; `features/killfeed.rs`,
`features/br_match/kill_report.rs`, `defeat.rs`, `elimination.rs`, `bots/fill.rs`; orchestrator/SCENARIOS.md (how to write a scenario).
Steps (one PR each; stop after each for review):
1. **SB-1 Tally + console `score`:** pure `Tally` in `features/scoreboard.rs` with the rules of SCOREBOARD §3 (kill = killer of a
   `Killed` event; death = `Killed` / `Tide`, plus LOCAL's and bots' mob deaths if Q1 = yes; downs not counted; left = marked, no kill;
   dedupe like `Feed`). Fed from `KillFeed::tick` for every accepted event (the bus topic keeps one reader). Frozen as "last match" when
   the expedition ends. A replacement bot takes the row and name of the bot it replaces (`bots/fill.rs`). Console `score` (dev build,
   `add-console-command` skill) prints one line per participant: `name K n D n [out: rival|tide] [left]`.
   Acceptance: unit tests for every row of §3; scenario **`score-bots`** (one game, `bots_fill = 5`, like `bots-match`): `bots_kill 1
   mob` then the replacement → `expect score` shows that bot's name again (and `D 1` if Q1 = yes); `bots_kill all` → `expect score`
   LOCAL `K 5 D 0`, every original bot `D 1`.
2. **SB-2 (done differently 2026-11-07: the blinking centre message on Esc / Start, not a toast; `score-bots` PASS) Own toast (option A):** on LOCAL's game only, after an event that changes LOCAL's kills or deaths, one gold toast
   `Your score: n kills · n deaths` (plural right, < 120 units with markup). Nothing about other players.
   Acceptance: `score-bots` gains `wait_log` "scoreboard: own toast" ×5 + "gametoast: shown" and a screenshot; new scenario
   **`score-self`** (one game, like `defeat-mob` then `defeat-rival`: `hp0` = mob death, wait the respawn, then `rival_hit` + `hp0`)
   → own toasts show deaths 1 then 2 (Q1 = yes) or only the second (Q1 = no); `expect score` LOCAL `out: rival`.
3. **SB-3 Mob / fall deaths on the bus** (only if Q1 = yes): kind 3 `Died` fact from the victim's game when `Defeat` judges a death
   that is not a defeat; no killfeed line; an older build ignores it (unit test). Acceptance: scenario **`mi-score`** (two instances,
   like `mi-killfeed`): P2 `hp0` with no rival hit (respawns), then `rival_hit` + `hp0` → on both games `expect score`: P2 `D 2 out:
   rival`, P1 `K 1`.
4. **(done 2026-11-07: the run-time text works in the dialog; it shows 3 lines, so the summary comes in pages) SB-4a Spike (Opus, budget one session):** a long placeholder (1024 units) in EventTextForMap_dlc01 and a `display: dialog` event in
   `m10_00_00_00` (`gamedata/emevd-events.json`); a dev command writes 8 coloured lines over the placeholder at run time and raises the
   flag at the Roundtable. Questions: does the dialog show the new text, how many lines fit, does markup work. Result into
   SCOREBOARD §9 and MEMORY_MAP. If the text does not change: stop, report, fall back to E2 (toasts in the banner).
5. **(done 2026-11-07, `score-bots-end` PASS; no colours, no team totals yet) SB-4b End summary (E1):** when the player stands in the Roundtable after a match, the dialog shows the frozen table (SCOREBOARD §6
   order and format; own row blue, rivals red; Duo/Trio grouped with team totals; "as of when you left" for an early leaver).
   Acceptance: scenario **`score-bots-end`** (one game, bots mode: `bots_kill all` → WIN → end-flow presses as `match-win-end` →
   Roundtable → `wait_log` "scoreboard: summary shown (6 rows)" + screenshot); `mi-score` gains the same end on both games with the
   same rows in both logs.
6. **SB-5 (later, Leonardo 2026-11-07):** a professional in-match UI by editing the game's GFx menus with JPEXS. **Spike done
   2026-11-07** ([re/GFX_UI.md](re/GFX_UI.md)): new HUD elements work; next GFX-1 (RE: the DLL sets a text in the HUD movie), then GFX-2.
Deliverables: `features/scoreboard.rs`, small edits in `killfeed.rs`, `kill_report.rs` / `defeat.rs` (SB-3), `bots/fill.rs` (names),
`gamedata/*.json` (SB-4), the four scenarios + SCENARIOS.md rows, CONSOLE.md row, testing-log entries, SCOREBOARD §9.
Acceptance (card): the scenarios above PASS; two real PCs (real hits, real names, the summary on both screens) go into the next FS
runbook (TESTING §6).
Out of scope: a persistent HUD counter (SB-5), lifetime stats across matches (Q4), assists, downs ("knocks"), ranking, any ImGui.
Result: (filled in per piece)

## Bots (Leonardo, 2026-11-06; design, plan and open questions in [BOTS.md](BOTS.md))

### BOT-R1 — RE: how to steer a bot
Owner: OPUS | Status: after BOT-1
Rank the four levers of BOTS.md §2b with static reading (AI target "host" field of logic 30000, plain-text logic loading, shipped
roaming logics) and one live probe each for the best two. Result into MEMORY_MAP and BOTS.md; BOT-3 is written from it.

### BOT-3 / BOT-4 / BOT-6
BOT-0 done 2026-11-06: host bots are not on a client; small-base invaders are (BOTS.md §4). BOT-6 builds friends + bots on the invaders.
BOT-2 done for Solo 2026-11-06 (`bots-match`); bots are on the killfeed as "Bot N" since 2026-11-07 (BOT-KF); their HP bar still reads `?NpcName?`. The early WIN while a replacement was owed is fixed (2026-11-07).
BOT-5 done 2026-11-07: the Bots switch in Matchmaking Settings (`mi-bots-menu`).
Brain v1, levels, friends + bots: written out when they start (BOTS.md §3 holds scope and order).

## Match flow: nights with no boss (M5; research `archive/research/M5-no-boss-nights.md`, `archive/research/M5-8-death.md`)

**Built earlier, since replaced by the RW rules (2026-10-24, below):** M5-1..M5-4 (no boss, no minions, clock console; engine facts TECHNICAL_NOTES §4e/§4f, MEMORY_MAP §16/§17), the elimination rounds M5-5 / M5-6 / M6-1, M5-8 research and M5-9 Solo real death. What stays: `features/br_match/` (match state), `game/deathhold.rs` (keeps a defeated player dead; SpEffect 102130 = die at once, the comeback can be held and released). Rule decisions: `DECISIONS.md`. A Solo die-at-once rule is open again: see "Solo night death" below.

### TIDE-MAX — The final Night's Tide circle closes as far as possible (no match timer)
Owner: OPUS | Status: review (Leonardo: look at the final-circle photo) | Depends on: — | Phase: match end
Goal: the zone forces the end of a match, as in Fortnite / Warzone, instead of a timer (DECISIONS 2026-11-07).
Rules: [re/PROJECT_SCOPE.md](re/PROJECT_SCOPE.md) §1 and §5: the final circle of Night 2 closes to a point; the tide's damage stays
vanilla; the most-kills tiebreak and any timer are out of scope.
Steps: (1) read every row of `PlayAreaCreateCommonParam` (MEMORY_MAP §16); (2) see what the game does at the night-2 fallback (600 s,
7507) with several teams alive (`tide-close-lab`); (3) `secondCircleRadius` 40 → 0 in row 0 (`gamedata/playarea-rows.json`, sbwriter now
writes `f32` fields and takes several rows files); (4) `clock` prints the live radius and the stage records.
Acceptance: `tide-close-max` PASS on one PC (records: final 0.0, first 347.6 (vanilla 367.7: the game derives it from the final radius); radius about 0.03 m at the night cap; LOCAL in the tide, 99811;
the tide defeats LOCAL at night) and `day1-free` PASS. Two-PC: the circle in a client's game (the circle is computed from the
synced clock and the same regulation, so it should match `[HYPOTHESIS]`): next FS.
Result: see testing-log 2026-11-07 "TIDE-MAX".

### M5-9 / M5-5 — superseded
The elimination rounds (night = a round, losers held until the morning, die-at-once in Solo nights) were replaced on 2026-10-24 by the RW
rules (DECISIONS "After the first real match"; section "Rework after the first real match" below). The hold (`game/deathhold.rs`) stays: it
keeps a defeated player dead. What FS-M59 left open (the hold in a real session, the other screen) is in the FS-RW1 runbook (TESTING §6).

## Hawk crossing (M7, replaces separate spawns; design in PROJECT_PLAN.md §13)

T-028 (teleport) is done: solo run 4 and the Nucleus two-screen check passed on 2026-10-14 (TESTING §3); code merged with `main` on
branch `t-028-snapback`. The F3 team-spawn rule built there is parked (`team_spawns = false`).

HAWK-0 (questions and desk research) is done (2026-10-21): answers and design in PROJECT_PLAN §13, engine facts in TECHNICAL_NOTES §4b
(hawk = player script state) and §4c (routes in `m60_00_00_99.msb`), tools in `archive/experiments/HAWK-0/`.

HAWK-A2 / HAWK-3 are done (2026-10-22, TESTING §3 HAWK-3 run 1): the env switch is found and swapped safely, `r8` = PlayerIns,
`hawk_drop` and `hawk_tree` work in the game (Leonardo). Design v1 that follows from it: PROJECT_PLAN §13 "v1 design". HAWK-1 (ride
probe) is dropped: the env trace answered what it was for. HAWK-1b is done (2026-10-22, TESTING §3 run 3: one diagonal line "B"
for every start route, three rides PASS); HAWK-4 is done (2026-10-22, TESTING §3 HAWK-4 runs 1-2: cruise lock, dive, start
animation cut; Leonardo: good enough). The line is built by `scripts\build-gamedata.ps1` (by default since HAWK-2; TECHNICAL_NOTES §4c).

### HAWK-2 — Hawk crossing v1 (player feature)
Owner: OPUS → SONNET/implementer → HUMAN | Status: **solo smoke PASS 2026-10-22** (TESTING §3; Leonardo: "how I wanted it").
Merged (PR #2). Open: the Spectral Hawk Tree check (a tree ride must behave as in vanilla), and the FS with two players; then remove this card.
Built: `game/hawk/` in both builds (`mod.rs` hook + swap + ride phases; `dev.rs` console and traces, dev build only). Settings
`[hawk]` (`enabled`, `dive_hold`, `dive_secs`, `dive_boost`, `start_cut`; defaults = HAWK-4's accepted values, on) read at
startup; the `hawk_*` commands are session overrides. Only LOCAL: once the main loop knows LOCAL's PlayerIns the hook acts on that
character only (before that, the HAWK-3 rider rule). A native tree ride (Spectral Hawk Tree) ends the lock; the pass-through swap
stays for the session (inert outside start rides). Route file: built by default (`-VanillaRoutes` to skip), required by
`package-release.ps1` and checked by the launcher. **Not a `Feature` yet** (ARCHITECTURE R3 open); moving it there = `configure` → constructor, `tick` → `Feature::tick` with `local` from the `World`.
What: PROJECT_PLAN §13 v1 as a player feature (cruise lock + dive + start cut with the accepted defaults; the console
commands stay dev-only): `hawk.rs` leaves the dev-only build, `[hawk]` settings in `nrpvp.toml`, on by default
in Battle Royale and offline mode, only LOCAL changed, swap removed when the expedition ends. The route file goes into the player
game-data package (lift the `map\` refusal for that one file). The first new feature, so the open halves of R2/R3 (typed views,
`Feature` trait; ARCHITECTURE §6) are built with it.
HUMAN: one solo smoke; then the next FS: both players cruise, dive and drop where they want; each sees the other land.

## Leonardo's list (2026-10-24, tracked here; update the state column when something changes)

Test routes: **1 PC** = automated scenario (`nro`, one game), **2 inst** = two real emulated-Steam instances on one PC (`nro`, `mi-*` scenarios,
[orchestrator/MULTI-INSTANCE.md](orchestrator/MULTI-INSTANCE.md)), **FS** = Friend Session on two PCs (TESTING §6).

| # | Item | Card | State (2026-11-01) | Still to test |
|---|---|---|---|---|
| F1 | Respawn far from all players | RW-7 | Built, unit-tested | Two instances: where each one starts and respawns after a mob death (2 inst), then FS |
| F2 | Runes shared only inside the own team (solo: himself) | RW-8 | **Built, `mi-rw8-runes` PASS 3/3 (2026-10-28, PR #25)** | Duo / Trio teammate case, a client-side kill, FS photo of both rune counters |
| F3 | No remote location (map, compass), name tags only on the target | RW-9 | **Built and seen with 2 inst**: map, compass, party bar, name tag, edge arrow, placed map pins with their 3D marker and green beam hidden for rivals (`mi-pin-beam`), pin toast off in every match (UI-5) | Duo/Trio teammates; FS photo |
| F4 | Enemy difficulty by team size (Solo / Duo / Trio values) | RW-10 | **Accepted: Leonardo found enemy strength balanced (2026-11-01).** Table rewritten on one PC (2 copies, 168 rows) | none beyond FS |
| D1 | Defeat / loss when killed by a player or the tide, then spectate or quit to the Roundtable | RW-1, M5-7 | **Done on 1 PC** (`defeat-rival`, `match-loss`, `spectate`, `esc-defeated`); spectate seen in Nucleus | 2 inst: the other screen (held body, defeat screen, spectate), real hits; spectator camera smoothness |
| D2 | Defeated players drop items (red phantom) on the ground; runes are discarded (DECISIONS) | RW-3 | **Weapons, talismans and picked-up consumables built** (`mi-rw3-items` PASS 2026-11-01; inventory part verified by hand, weapon level kept); runes discarded | 2 inst |
| D3 | Death by mob / boss respawns normally with items | RW-2 | **Done on 1 PC** (`defeat-mob`) | none beyond FS |
| D4 | Boss dormant powers / weapons only to the last hitter's team | RW-5 | **Parked 2026-10-31, vanilla (see RW-5)** (`disableMultiDropShare` param hint) | RE first, then 2 inst with a boss |
| D5 | Day 1 / night 1 free of the tide, tide starts on Day 2 | RW-4 | **Done on 1 PC** (`day1-free`) | FS (real clock over the network) |
| D6 | Match ends when one team is left, even on Day 1 | RW-1 | **Done on 1 PC** (`match-win`, `match-win-end`, `match-loss`) | 2 inst with real defeats; FS |
| D7 | Private match by password, outside matchmaking | RW-6 | **`mi-private-room` + `mi-private-mismatch` PASS on two instances (2026-11-11)** (`private_code` in `nrpvp.toml`); the in-game "Multiplayer Password" field is the planned player route (RW-6 follow-up) | 2 inst: same code meets, different codes do not (`mm_queue` works over the emulator, `mi-queue`) |

Backlog since 2026-11-02 (Leonardo: nothing here is scheduled; UI improvements come first, only simple work with a valid result). Order when picked up again (2026-11-01): **Sites of Grace blocked in PvP combat**, **Duo and Trio first tests**, then the 2-instance checks of F1, D1, D7. Open ideas without schedule: Solo night death, match time limit. Arena is in design since 2026-11-08 (ARENA-0).

## Not built yet (Leonardo, 2026-11-01; rules in `re/PROJECT_SCOPE.md`)

### DUOTRIO-1 — First Duo and Trio tests (Leonardo 2026-11-02; now the official Duo / Trio work)
Owner: HUMAN + OPUS | Status: **in progress**: `mi-duo-party` (party, queue, expedition as `ally`) PASS 2026-11-02; next scenarios on that base (two teammates): runes shared, revive, HUD / pins of a teammate, a teammate's defeat; then a puppet team scenario for 2 vs 2 / win by team | Depends on: none
Duo / Trio have never been tested by us (teams of 2 and 3, teammate revive, shared runes, HUD filter, win by team). **Must check:
a downed teammate shows the near-death gauge** (rivals' stays hidden, DECISIONS 2026-11-07). Write `mi-duo-sc` (NRO-MI) and a 4-puppet team scenario; list what breaks. MM-6 covers matchmaking for teams.

### SOLO-REVIVE — Solo night death with a revive item (BACKLOG, Leonardo 2026-11-02)
Owner: OPUS | Status: idea, no schedule (DECISIONS 2026-11-01)
PROJECT_SCOPE §1 wants a Solo kill at night to be a real death at once unless the player carries a revive item (Wending Grace, goods `0x400002BC`) that then works as in vanilla. First step is only a test: with and without the item, does the current build already behave so? Only then a card for code.

### TIMELIMIT-1 — Match time limit (idea)
Owner: OPUS | Status: idea, no schedule. PROJECT_SCOPE §1: "if a match passes the maximum vanilla time everyone loses". Not built.

### ARENA-0 — Arena feasibility spikes (Leonardo 2026-11-08; rules PROJECT_SCOPE §Arena)
Owner: OPUS (RE) + HUMAN (in-game looks) | Status: **spike 1 PASS solo; spike 1b (a)(c)(d) PASS with two instances (2026-11-08, branch `arena-0-spike`); (b) and spikes 2-6 left** | Depends on: none
Arena (MODE-02) is built in slices; each unknown below gets a time-boxed spike before code. Findings: MEMORY_MAP §27.
1. **Enter Heolstor's arena — PASS solo (2026-11-08, testing-log).** The vanilla Day 3 chain is not usable by flags (TRAPS); our event
   99100900 (`gamedata/emevd-arena.json`, dev only) runs Warp Player `2003[93]`(m18_00_00_00, 18002200) on our flag 10007500 and took
   Leonardo from Day 1 of a Night Aspect expedition into Heolstor's arena in ~1 s. Arena: flat, centred on (0, 0, 0), ≥ ±150 m.
   **Arena card — shows (2026-11-08, testing-log):** `NightBossMenuParam` row 17 "Arena" (sortId 2, right after Battle Royale) is
   listed by a hardware breakpoint on the card list loop (`arena_card hook`, MEMORY_MAP §28; dev, hwbp slot 1). **Commence on it → straight into an
   empty Heolstor arena: PASS solo (2026-11-08, testing-log)**: the Commence handler remembers the Arena card, the main loop sets
   flag 10007500 3 s after LOCAL appears; Heolstor is kept off by `gamedata/emevd-arena-m18.json` and his fight never starts (`emevd-arena-func.json`: no 7511, no boss bar, 3 min checked). The hook holds hwbp slot 1 only at
   the Roundtable with the queue idle (the quiet invite needs it only after a match is found). Left: the player build (hook + event
   files are dev only), a queue test with the menu open.
   **For ARENA-1 (Arena queue, player build) — Leonardo 2026-11-08:** the hook must stay armed while an Arena search runs (else a
   reopened menu loses the Arena card mid-search) and give slot 1 back only when a match is FOUND (the quiet invite's moment).
   Acceptance then: `mi-zero-click` PASS with the hook in the build, plus a solo check: Arena chosen, search, close and reopen the
   menu, the card is still there, Stop Searching works.
   **1b:** (c) and (d) **done statically (2026-11-08, MEMORY_MAP §27)**: Heolstor is a map character (no spawn event; our disable
   holds), only his death (7512) ends an `m18` match, so the Arena ends it with our per-game endings (flags 10007411 / 10007412).
   **(a) + (d) PASS live (2026-11-08, two instances, testing-log):** both players warped from one BR expedition stay in one
   session in the arena, in sync, and can hit each other; WIN / LOSE banners play per game in `m18` and both return to the
   Roundtable. Left: (b) a save that never unlocked Heolstor (the warp loads the map file every install has `[HYPOTHESIS]`).
2. **Spawn points and round reset** `[spawns chosen, reset UNKNOWN]`: default spawns A (−30, 0, 0) / B (+30, 0, 0), 60 m apart,
   facing each other (agent's default, Leonardo: tune later); teleport, full restore of HP / FP / flasks / skill cooldown without a
   map reload, **while the ultimate gauge is kept**; the game's maximum flask count.
3. **Level 15** (**found 2026-11-09, MEMORY_MAP §29**): the game's setter `exe+0x209E20(PGD, level)` writes `+0x68` and recalculates the
   attributes; only the grace level-up menu calls it. Left: call it with 15 in the arena and check HP / FP refill and that it holds.
4. **Max upgrade / legendary** (**answered 2026-11-09, MEMORY_MAP §29**): rarity is a fixed weapon field; the upgrade level is the id's last two digits and its maximum is the top `ReinforceParamWeapon` row of the weapon's `reinforceTypeId` set (common +3 .. legendary +0; Nightfarer items +14), readable from the param in memory. Was: the
   top level per weapon, and whether a Nightfarer's unique default weapon has one.
5. **Sparring loadout** (**answered 2026-11-09, MEMORY_MAP §29**): the Sparring Equipment gives real inventory items; the
   expedition start wipes them. Read at Commence: the six hand slots' ids and their instances' `+0x18` (custom row) / `+0x1C`
   (skill), the quick slots, the consumables. Give after the warp, on the game's thread: add -> equip -> write `+0x18` / `+0x1C`
   (verified: Uchigatana with Seppuku, staff with spells, +3 with Seppuku). Left: passives (`+0x20..`, none seen yet), whether the
   HUD needs a refresh after our equip, relics applied after the warp.
6. **Leave the session after the match** `[partly known: FS7-8 "F10 also leaves the session"]`: winner VICTORY, loser defeat, then both
   at the Roundtable out of the session.
Then slices: ARENA-1 card + queue (2 players) + arena entry + facing spawns (**done 2026-11-08**, testing-log); ARENA-2 rounds (best of 3) and the end screens (**done 2026-11-08**, testing-log; the ultimate only in the last round works, #73, Leonardo 2026-11-08; open: one near-death message before "fight", the 3-minute round limit + HP tiebreak is not built); ARENA-3
level 15 + loadout + max upgrade + round limit (**done 2026-11-09**, solo + `mi-arena-loadout` PASS, testing-log; open: passives, the flask
count, the quick-slot order, the player build); ARENA-4 offline 1v1 against one level-13 bot (**done 2026-11-10**, `mi-arena-bot` + Leonardo's match PASS, testing-log; bot level 13 kept by Leonardo; an Arena WIN counting as an Everdark win is accepted (DECISIONS 2026-11-10); the end summary now shows the Arena round score, SCOREBOARD 2026-11-10); ARENA-5 Duo / Trio. One PR at a time; one PC with two instances shows ARENA-1..4.

### ARENA-FS12 — Three Arena bugs from the 2026-11-12 test: rival's weapons, mid-round respawn, solo run after a match
Owner: OPUS | Status: **built (branch `arena-fs-fixes`), `mi-arena-loadout` and `mi-arena-requeue` PASS on one PC (2026-11-13,
testing-log)**; waiting for a real-Steam Arena | Depends on: ARENA-LV (its real-Steam part passed 2026-11-12: max HP held)
Fixes: (A) the game's player-data broadcast `exe+0xDE20E0` after our loadout / level resets / round-1 start, so the rival draws our
real weapons; (B) one get-up write, a retry only after 2.5 s (a second write mid-get-up made the game kill and respawn the player with
a level lost); (C) at the Roundtable after a match, the game's lobby left over with us alone in it is left (`exe+0xDFD940`), so the
next card pick is our Commence again (was MM-REQ).
Acceptance (next real-Steam Arena, both players' logs): each player sees the rival's chosen weapons (console `rival_gear` in a dev
build, or by eye); no `arena_level: slipped` line after a lost round and no mid-round respawn; after each Arena both players queue
again with one Commence (no `session: role none -> other` without a `Commence pressed` before it), as host and as client, in both
return orders.

### ARENA-5 — Arena Duo (2v2) and Trio (3v3) (Leonardo 2026-11-24; rules DECISIONS "Arena Duo / Trio rules")
Owner: OPUS | Status: **built (branch `arena-duo-trio`); 2v2 PASS on four instances (2026-11-24, testing-log: round 1 by hand with
real hits, then the scenario's three rounds step by step)**; open: an unattended `mi-arena-duo` run, Trio, real Steam | Depends on: MM-FILL (packing, roster teams)
Built: the Arena card queues Duo / Trio (`menuflow`, a short party is filled); `arenarounds` judges whole teams (`step_teams`: lost
when LOCAL and every teammate are beaten, won when every rival is; a beaten player stays beaten for the round; a leaver is simply
missing), a beaten player with a teammate still up is frozen on its team's bench (`Do::Bench`, `arenacard::bench`); the time limit
compares each side's mean HP; the host's verdict carries its roster team (`Outcome::wire`); teammates spawn 4 m apart by roster slot
(`mm::local_roster_slot`). `nro` takes 4 instances (instance 4's save made once by hand). Console `mm_queue_arena_duo/squad`.
Acceptance: `scenarios/mi-arena-duo.toml` PASS (four instances: bench, team rounds both ways, 2-1, WIN / LOSE per team); the next
Friend Session with 4+ players plays a 2v2 and, with 6, a 3v3 (TESTING §6). The bench seat is replaced by the team spectate (ARENA-6).

### ARENA-6 — Arena: watch the team after a beating; Duo / Trio against bots (Leonardo 2026-11-24; DECISIONS of that date)
Owner: OPUS | Status: **built on branch `arena-duo-trio`**; hidden-body lab done (2026-11-25, testing-log: the disable hides only on
its own game and freezes the body's position for the others, so the body is hidden on the far bench before it follows); open: a 2v2
on four instances PASS played by hand (2026-11-25, testing-log `2026-11-25-arena-duo-spectate`); Trio against bots PARTIAL (2026-11-25,
v0.4.2, testing-log `2026-11-25-arena-trio-bots`): round 1 3v3 fine, **round 2 got 1 of 3 new enemies** (the handle of the bot not
needed in the first fill came back on a new bot, which was skipped as old; fixed after the run, unit test only); open: a Trio re-run with the fix, a Duo match against
bots, the spectate against bots, the switch inputs | Depends on: ARENA-5
Built: `Do::Bench` became `Do::Spectate`: frozen, flags 10007521 (event 99100921: no collision, no gravity) and 10007522 (event
99100922: Change Character Enable State Disabled) ON while watching (`spectating_now`: from the beating to the reset; saved flags,
written on a session's first tick), the body teleported onto a living teammate (player or bot ally) every tick (`pick_watch`, mouse
buttons / D-pad switch); nobody to follow = the bench. Console `arena_hide 0|1`. Bots: `Decision::PassArenaBots(size)` (the players
byte set to one so the game runs its solo Commence), `mm::arena_bots_size`, the director keeps 2n-1 bots with places (`fill::Role`:
enemies first, then allies), ally team 2 / enemy team 6, parks on opposite sides, replaces the dead at the reset; `arenarounds` takes
bot allies as teammates and enemy bots as rivals.
Acceptance:
1. **2v2 on four instances** (`mi-arena-duo`): the beaten player's log has `on the bench; hidden in`, `spectator body hidden on`,
   `watching teammate`; its screen shows its teammate, a switch changes it; the other three see it only on the far bench; it lands on
   its spawn at the reset (`spectator ghost ... off`, `pos` on the floor). A body seen on the teammate by others = the real-death
   fallback.
2. **Duo against bots (one instance, offline):** Bots On, the Arena card, Duo, Commence: `kept bot ... Arena enemy slot 0/1, ally
   slot 1`, the ally fights the enemies and never LOCAL, the two enemies do not fight each other, `hp_set 1` on LOCAL = watching the
   ally, rounds and WIN / LOSE as in Solo. Then Trio.

### ARENA-ULT-DMG — Some ultimates deal no damage to a rival player (Leonardo, 2026-11-17)
Owner: OPUS | Status: **open: which ultimates is UNKNOWN** (ask Leonardo) | Depends on: nothing
Static check done (2026-11-17, regulation 1.03.3 + Smithbox 2.2.6 defs and row names, read-only exporter in the scratchpad): the
ultimate rows (`Bullet` 3090xxxx, `AtkParam_Pc` 7000001xx-7000004xx, Executor `Bullet` 10135-10700) look like attacks that do hit
players: `opposeTarget` 1, `friendlyTarget` 0 everywhere; `Bullet.isHitBothTeam` 0 and `dmgCalcSide` 0 on every row of the game,
working ones included; their SpEffects (706100, 706120, 705022, 704100) target Player and AI alike. What differs: ultimates have
`atkPhys` 0 with `subCategory1` 123 (Ultimate Art), `isPenetrateChr` 1, and Recluse / Revenant work through SpEffects, not damage.
So the cause is in the engine, not in a param flag `[HYPOTHESIS]`.
Next: (1) Leonardo names the ultimates that did not hurt (and whether the rival flinched); (2) two-instance lab: the ultimate on
the rival with `ult full`, rival HP before / after, per Nightfarer; (3) if only some fail, compare those rows with the working ones;
try the rival's copy at team 6 (Enemy) instead of 29 in the Arena only (MEMORY_MAP §4) as the cheapest live test.

### ARENA-CLOCK / ARENA-OT — Round time left on screen, and overtime (Leonardo 2026-11-17)
Owner: OPUS | Status: **solo against the bot PASS (2026-11-17, testing-log 2026-11-17-arena-clock-overtime); left: two players in
overtime (next Arena FS)** | Depends on: none
- **Clock:** the rune counter (top right) shows the round's seconds left while an Arena match is on (`game/arenaclock.rs`; runes have
  no use in the Arena): the limit before a round, counting down while it is fought (0 in overtime), frozen at the round's end, the
  full limit again from the reset; the old rune count is put back at the endings. Console `arena_clock [0|1]`. A real text clock
  (GFX-1/2, re/GFX_UI.md §5) can replace it later.
- **One minute left:** the game's message "One minute left!" once per round (only with a limit above 60 s).
- **Overtime (DECISIONS 2026-11-17):** at the limit "Overtime! HP drains until someone falls"; each game drains its own player 2 %
  of max HP per second (and the bot), never below 1 HP; the first at 1 HP loses (the host judges); after 60 s the HP % rule.
Acceptance (one PC, against the bot; `arena_limit 70` for a short round): the counter counts down; "One minute left!" at 60 s
left; at 0 the overtime message, both HP bars fall about 2 % a second, the one lower at the start reaches 1 HP first and loses
the round ("time! you won/lost"); the next round starts at the full limit; after WIN / LOSE the counter is back to the old count.
Two PCs (next Arena FS): both games agree on an overtime round.

### RW3-BIGINV — Defeat drop skips talismans and consumables on a save with many relics (found 2026-11-09)
Owner: SONNET/implementer | Status: open, small | `defeatdrop::read_inventory` rejects a list with `used > 0x180`, but Leonardo's
list holds ~609 entries in an expedition (553 relics, MEMORY_MAP §29), so a rival kill drops only the equipped weapons. Fix: the
bound `invspy` uses (`used <= 0x1000`, keep the `+0x3BC == 0x180` check). Acceptance: `mi-rw3-items` PASS and a unit test.

## Rework after the first real match (RW; Leonardo's notes 2026-10-24, rules in DECISIONS)

Built and tested on one PC 2026-10-24 (scenarios `day1-free`, `defeat-mob`, `defeat-rival`, `match-win`, `match-win-end`, `match-loss`;
testing-log): **RW-1** defeat for good + early win, **RW-2** a non-player death is vanilla, **RW-4** free Day 1 (the code is in
`features/br_match/mod.rs`, header comment). Built, unit-tested, not yet seen on two PCs: **RW-6** private match, **RW-7** respawn far, **RW-10**
enemy scaling. Their two-PC checks are in the FS-RW1 runbook (TESTING §6); the cards come off when FS-RW1 passes. Open, each waiting for
what a second real client shows (research tools: console `memscan`, `peek`; findings so far below):

- **RW-3 — Defeat drops.** **Slice 1 built, `mi-rw3-items` PASS on one PC (2026-11-01):** a rival-caused defeat drops the victim's equipped **weapons** at its spot; the host makes the drop with the game's own function on the game thread (`game/itemdrop.rs`, `game/defeatdrop.rs`, bus topic 5), the game shows it to every player and any can pick it. **Slice 2 (2026-11-01):** talismans and picked-up consumables are read from the inventory list and dropped with the weapons (not the flask); verified by hand on the host as victim, not in a scenario (key presses into the second window are unreliable). **Left:** a scenario for the inventory part, weapon art / Grief effect (the weapon LEVEL is kept: it is part of the id, verified 2026-11-01), item lifetime, the victim's inventory emptying, the client-initiated drop path, the two-PC check (FS-RW1). Runes are discarded, not dropped (DECISIONS 2026-11-01). The red phantom figure is scrapped (DECISIONS 2026-11-01). Research: `research/RW3-handover.md`, `RW3-red-phantom-*.md`, MEMORY_MAP §24.
- **RW-5 — Boss orb: first to loot — PARKED, vanilla ships** (Leonardo, 2026-10-31). Built and seen on two instances but **off by default**: `game/orbclaim.rs`, console `orb_auto 1` + `orb_list / orb_off / orb_on / orb_claim / orb_go / boss_orb`, topic `ORB_CLAIM`, scenario `mi-rw5-orb` (switches it on). **Why parked:** a client's pick needs the host's copy of the orb, so nulling the host's copy blocks a client claimer too (MEMORY_MAP §22); the fix is a hook at the game's interaction call site (static RE; a hardware read watch on the slot crashes the game, TRAPS). Also open: second kill overwrites slot +0x58 (an unpicked orb is lost, the rescue-by-moving experiment failed), orb lifetime, nulled slot at expiry, two real PCs. **If resumed:** time-box the call-site hook spike; if no clean block point, stay vanilla.
- **RW-8 — Runes only inside the own team.** **Built and accepted on one PC** (PR #25, 2026-10-28): every game credits its own player for every death it processes (`exe+0x3DE710` → `exe+0x668380`, no killer check, MEMORY_MAP §21); the killer's game records the attacker (`ChrIns+0x1A0`), the other game reads `-1` (it applied no hit). `game/runeshare.rs` (both builds) passes amount 0 when the last attacker is a rival, or — with no teammate (Solo) — when none is recorded. Console `runeshare`, `runes_mark`, test aids `mobs`, `mob_go`, `mob_kill`. **Acceptance:** `scenarios/mi-rw8-runes.toml` **PASS 3/3** (runs `1793219963/-969`, `1793220173/-180`, `1793220436/-441`, 2026-10-28): player 1 `+1040` / `0 blocked`; player 2 `+0` / `1 blocked = 100 rune(s)` and `Unknown -> BLOCKED`; control `runeshare 0` pays player 2 (`+1040` / `+1560`). **Open:** Duo / Trio teammates (an unrecorded killer stays vanilla there — it may be a teammate's; needs the killer from the owner's sync, UNKNOWN); the message credit `exe+0x51DC0B` (enemy not loaded here) is counted, not filtered; FS-RW1 step 7(a) on two real PCs (photos of both rune counters). Remove the card when the FS check passes. **Review 2026-10-30 (code only, 494 unit tests pass):** hooks are checked before patching and fail safe; **untested risk:** in Solo a *client* who kills an enemy owned by the host sees no attacker recorded (`Unknown`) and gets no runes — the scenario only has the owner kill. Check with a client-side real kill (or find where the host's verdict reaches the client) before FS-RW1.
- **RW-9 — No remote map / compass position, nametags only on the target.** **Built for every team size (the Duo / Trio teammate case is still open), seen with two real instances on one PC (2026-10-27, branch `test/rw9-multi`):** `game/hudfilter.rs` hooks three HUD lookups (MEMORY_MAP §20 "Rival filter") so a rival is on no map pin, compass mark, party bar or name tag (incl. the edge arrow); the lock-on target has no name tag either (the game's enemy bar names it, 2026-11-06); teammates are not rivals and stay. `maphide` (Solo, whole list) is only the fallback. Console `rivalhud`, test aid `tp_player`. Life bars from lock-on / hits stay vanilla (DECISIONS). Seen: with the filter on, no party bar / name tag / edge arrow on both screens (run `1793065578`, `h1_hawk`, `h2_hawk`); compass and map pin gone with `maphide` (run `1793064664`). **Acceptance:** `scenarios/mi-rw9-rivals.toml` PASS 2/2 (2026-10-27). **Map markers (2026-10-27, branch `rw9-markers`):** a pin a rival places arrived and showed on the map, compass and as a 3D marker; two more hooks (MEMORY_MAP §20 "Map markers") now keep a rival's pins off all three (own and teammates' stay), `scenarios/mi-rw9-markers.toml` PASS (testing-log 2026-10-27). Still shown: the "Ally is showing location" toast with a landmark icon (producer UNKNOWN). **Left after that:** the toast, Duo / Trio teammate case (needs `team_size = 2` over the queue or a 4-instance PC), FS photo on two PCs. Remove the card when a teammate run shows the teammate still marked. **Open:** the toast "Ally is showing location" still reveals the area to a rival; notes and plan (and the killfeed idea built on the same mechanism): [re/TOAST_KILLFEED.md](re/TOAST_KILLFEED.md).
- **Spectating (M5-7) is built in v1** (STATUS; DECISIONS 2026-10-25): the held body is moved next to a living player. Camera smoothness and the two-PC view are the open items below.
- **RW-6 follow-up:** the game's own Matchmaking Settings has a "Multiplayer Password" field; the text lives in a `CS::CSMenuSimpleSaveDataChunk<3, MENU_INPUT_HISTORY>` object (input history, found with `memscan`) and in a second structure; which one the menu reads at Commence is UNKNOWN. Reading it would replace `private_code` in the toml (no config editing for players).
- **Spectator camera smoothness (next):** the moved body makes the camera jump about 20 times a second. Idea: find what writes `ChrExFollowCam+0xA0..` (the followed position, newest entry; console `trace_write <addr>`) and feed it another player's position or target directly, leaving the body where it died. Fallback: keep the moved body (committed, 2026-10-25).
- **ESC / pad while defeated (works on one PC, 2026-10-25; scenario `esc-defeated` PASS, pad confirmed by Leonardo).** `game/menugate.rs`: Esc or pad Start opens the game's menu for a defeated, held player (sets the open request, MEMORY_MAP §19); pad d-pad right / left switch the watched player (mouse buttons too; all off while a menu is open). The pad is read from the game's own input bits (XInput is blind inside the game). Quit through the menu while dead works (one PC, Leonardo). Left: a Nucleus / FS confirmation, other pad layouts (bits measured on a Steam Input Xbox pad). Remove this card after that.

## Ideas from FS-RC1 (2026-11-01, testing-log; none scheduled)
- **Decide the win at once:** publish a `DEFEAT` fact on `net::bus` from the victim's game so the winner does not wait the 30 s `OUT_DEAD_MS` after a rival kill (needs a topic, key = team, a test in the golden traces).
- **Quiet the `mm_join: gate:` log** after the expedition has started (about 150 identical lines per match).
- **Double press of Commence** cancels the search (toggle); consider ignoring a second press within a second.


## Oddities recorded by the golden traces (R7-1, 2026-11-01; behaviour kept as-is, decide later whether any is a bug)

1. The event-file fallback re-fires `GameFlag(7000/7001)` every 1500 ms while our flag stays ON in the world.
2. With no positions known, the spectator logs "spectator switched to player..." but sends no `Follow`.
3. A mob-death respawn with no positions of others logs "respawned inf m from the nearest player; no move".
4. The mob-death log prints "last rival hit None ms ago" when there was none.
5. Night flag ON on Day 1 with the clock past 11:50 makes the host set 7502 at once.
6. A LOCAL dead (HP 0, not downed) for 30 s counts as *out* even when a mob's death would respawn it (`OUT_DEAD_MS`, by design).
7. After the expedition-leave reset `status` is empty until the next in-expedition tick.
8. `DeathHold(false)` and `MenuGate(false)` are emitted on every leave, even when neither was on.

## Cloud (CLOUD-n): player logs, match data (Leonardo 2026-11-11; design [CLOUD.md](CLOUD.md))

Order: CLOUD-1 and CLOUD-2 before the open release; CLOUD-3 once people play; CLOUD-4 later. One PR at a time.

### CLOUD-1 — Log upload server (C#, Lightsail)
Owner: SONNET/implementer → OPUS review; HUMAN deploys | Status: todo (needs: where Postgres runs, CLOUD §2) | Phase: before the open release
Goal: `POST /v1/logs` stores a player's log zip and answers a short code; `GET /v1/health`.
Context: CLOUD.md §2-§5.
Steps: 1. `cloud/NightreignPvP.Api` (ASP.NET Core Minimal API, .NET LTS), `cloud/NightreignPvP.Api.Tests`. 2. EF Core + Npgsql, table
`log_upload`, the first migration. 3. The route: validate `meta` (schema 1, sizes), stream the file to
`<root>/<yyyy>/<mm>/<code>.gz` (gzip of the zip), sha256, row, `201 {code}`; codes: 6 chars from `ABCDEFGHJKMNPQRSTUVWXYZ23456789`,
retried on a clash. 4. Rate limit per IP (`AddRateLimiter`: 5 uploads / 10 min), 8 MB limit. 5. `BackgroundService`: delete files and
rows older than 30 days, once a day. 6. Config by environment (connection string, storage root); `cloud/README.md`: build, run, deploy
(systemd unit + Caddy for HTTPS), back up. 7. CI job `dotnet test`.
Deliverables: `cloud/`, CI change, CLOUD.md updates (URL, what is deployed).
Acceptance: `dotnet test` green with integration tests (WebApplicationFactory + a throwaway Postgres): upload → 201 + file + row;
too big → 413; bad meta → 400; the 6th upload in 10 min → 429; retention removes an old row and its file. HUMAN: deployed on the
Lightsail, `curl https://<host>/v1/health` → 200, one `curl` upload returns a code.
Out of scope: matches, accounts, a web page to browse uploads (psql is enough).

### CLOUD-2 — "Send logs" in the launcher
Owner: SONNET/implementer → OPUS review | Status: todo | Depends on: CLOUD-1 (a local run of the API is enough to build it) | Phase: before the open release
Goal: one click in the launcher uploads the player's logs and shows the code to paste on Discord.
Context: CLOUD.md §3-§4, `crates/nrpvp-app` (RELEASE §5b).
Steps: 1. `install.id`: a random UUID made once in `Game\NightreignPvP\`. 2. Collect: `nrpvp.log`, the 3 newest `logs\nrpvp_*.log`,
`launcher.log`, the me3 log, `VERSION.txt`, `nrpvp.toml`; skip what is missing; cap 30 MB before compression. 3. Replace every
SteamID64 with a salted hash (one salt per upload). 4. Zip, `POST /v1/logs` with `meta` (`note` from an optional text box), API URL
constant + `NRPVP_API_URL`. 5. UI: a "Send logs" button (Help / bottom bar), progress, then the code with a Copy button; errors as
a toast; a mock in `launcher-ui-mock`.
Deliverables: `crates/nrpvp-app` changes, RELEASE.md (one line), CLOUD.md (what the launcher sends).
Acceptance: unit tests for the file list, the size cap and the Steam ID hashing (same id → same hash, no `7656119…` left); upload
against a local API returns a code and the zip on disk holds the files; HUMAN-solo: the button in the built launcher shows a code.
Out of scope: automatic uploads, crash dumps.

### CLOUD-3 — Match summaries from the mod
Owner: OPUS design → SONNET/implementer | Status: todo | Depends on: CLOUD-1 | Phase: after the open release
Goal: at the end of every online match each game sends its own summary (`POST /v1/matches`, CLOUD.md §4); `telemetry = false` in
`nrpvp.toml` turns it off.
Steps (outline): the route, tables `match_report` / `match_player` (CLOUD-1's server); in the mod, the summary from the killfeed /
scoreboard (BR) or `arenarounds` (Arena) at the expedition end, sent from a background thread (never the game thread), dropped after
one retry; `match_key` = hash of the queue lobby id; pids = hashed short ids. Bots matches are not sent.
Acceptance: unit tests (payload), `mi-arena-rounds` and one BR scenario show `telemetry: match sent` against a local API.

### CLOUD-4 — Optional accounts (Steam sign-in, Discord link)
Owner: OPUS | Status: backlog (after the open release, when people play) | Design: CLOUD.md §6.
