# Decisions made (so they are not re-argued)

One line per decision with date and who decided. Product intent: `re/PROJECT_SCOPE.md`; engineering decision records with evidence (ADRs): `TECHNICAL_NOTES.md`.

- Stack: Rust DLL loaded through me3 next to Seamless Co-op; nothing in Seamless Co-op is modified.
- **The table offers only Solo / Duo / Trio Battle Royale** (Leonardo, 2026-10-19): no normal expedition while the mod is installed. The
  choice opens the stock boss menu; pressing start begins our matchmaking; the game's own "Cancel matchmaking" stops it (no Leave entry). UI-1.
- **No ImGui for players in the final product** (Leonardo, 2026-10-19): menus and status messages must be the game's own UI. The
  Insert panel was a stop-gap; **removed from the player build on 2026-10-20** (the overlay is a developer tool only).
- **Every match has 6 players** (Seamless Co-op's limit; Leonardo, 2026-10-19): Solo 6 × 1, Duo 3 × 2, Squad 2 × 3. Pre-release zips use 2 for testing.
- PvP is always on for rivals; players cannot switch it off. Launching with `nrpvp_launcher.exe` is the mode, until a menu entry exists.
- Hostility: each game keeps itself and teammates on the normal value and marks only rivals (value 29).
- Downed players: teammates fill the purple bar; rivals shorten the bleed-out and never move the bar. Instant death on 0 health was
  rejected on 2026-10-12 (auto-revive items would defeat it); superseded for Solo nights on 2026-10-23, reversed again 2026-10-24 (see \"Downed always\" below); the wish for a Solo die-at-once rule is open again (2026-11-01 entry).
- Investigation first, then building: confirm memory locations with the scanner before adding mod code.
- Matchmaking joins through the game's own lobby (Invite Members path), never a lobby our mod creates; never EAC, never Bandai servers.
- Leonardo only does smoke tests; Claude decides design and order, and does not build tools that are not needed for the goal.
- **Developer control is a text console** (2026-10-20): commands over a named pipe with `nrcon`, so agents can drive a running game;
  the ImGui panel and F-keys were deleted (R5, 2026-10-20). `CONSOLE.md`.
- **Code structure** (2026-10-20): ports and adapters, typed memory views, feature modules, dev code behind a cargo feature; refactor
  before the next big features. `ARCHITECTURE.md`.
- **The crossing line** (Leonardo, 2026-10-22): one line for every start route, diagonal "B" of the route atlas (the longer and higher;
  the "bottom to top" rule is dropped). TASKS HAWK-1b.
- **Superseded (2026-10-24, \"Downed always\"). Solo: a kill is a real death** (Leonardo, 2026-10-23; PROJECT_SCOPE §2): at night 0 HP without a revive item = dead at once;
  the dead spectate; Day 2 respawns everyone at random places. Research first (TASKS M5-8), then M5-9. Kill feed: Leonardo's own
  later system (bottom-right notifications), not now.
- **Superseded for players killed by players (2026-10-24, RW rules). Eliminated = held down, not dead** (Claude, 2026-10-23; implements M5-D; stays for Duo/Trio and as the fallback): the game's death and ~7 s comeback are engine code in
  an obfuscated region, so an eliminated player is kept in the downed state (clock held) until the morning and then stood up with
  the game's own get-up. Spectating someone else is a separate camera card (M5-7). **Solo uses the real death instead** (M5-9,
  from M5-8: SpEffect 102130 + the held comeback). Esc while dead: not now; M5-7 must at least allow quitting (Leonardo, 2026-10-23).
- **Match start = hawk crossing** (Leonardo, 2026-10-21): the start hawk flies one straight line across the whole map and each player
  jumps off where they want (Fortnite style). Replaces separate team spawns. `PROJECT_PLAN.md` §13.
- **Matchmaking direction** (Leonardo, 2026-10-21): stay on Seamless Co-op as long as it carries us, with three must-haves: no
  Steam window or invite ever on a player's screen, everyone arrives ready and the match starts by itself, and the "host" stops being a
  player-facing role. Asking SC's author is out of scope. Studying how SC works (read-only, for interop; never patched, copied or
  redistributed, hard rule 3) is approved, to prepare our own matchmaking/session layer if SC becomes the roadblock.
- **Superseded 2026-10-24 by \"After the first real match\". Nights are elimination rounds** (Leonardo, 2026-10-22; `re/PROJECT_SCOPE.md` §2): no boss, no minions; no respawn at night,
  the dead spectate; a night ends only when one player (team) is left, no timer; only that winner is healed; the others respawn
  spread out at the morning; night 2's winner wins the match, the rest lose; no Day 3 / Nightlord; the 40 m final circle stays.
  At night 0 HP = knocked for the normal 15 s (Solo too; rival hits −3 s, teammates revive); bleed-out over = eliminated until
  the morning, spectating (M5-D, 2026-10-22).
  Research and tests come before code: Leonardo runs Nucleus multi-instance checks for it before the first UAT with his friend.
- **Host leaving mid-match** (Leonardo, 2026-10-21): for now accept that the match ends, and end it cleanly; the real goal is **host
  migration** (Destiny / old CoD style), worked on as far as it goes. The host is picked by the matchmaker (best connection), never by
  luck, and has no player-facing role. **A server host (a PC running the game with no player) is rejected** for now.
- **One owner doc per topic** (2026-10-20); the map is in `CLAUDE.md`. Finished task cards and runbooks leave the docs (git keeps them).
- **Bots are not a game feature; Nucleus is a secondary tool; the orchestrator is in this repo** (Leonardo, 2026-10-24): the automatic offline bots (OFF-5) are gone; the game's bot manager stays as the engine of the orchestrator's puppet players. `scripts\deploy-nucleus.ps1` stays for quick two-window checks of plain rules (no event files, no `-Me3`); anything that needs our event files runs through `nro` (offline, puppets) or a friend session. `nro`, its scenarios and tools live in `crates/nro*`, `scenarios/`, `tools/`, `pins/`.
- **Docs are small files with one owner each** (Leonardo + Claude, 2026-10-24): STATUS holds only "now" (≤ 8 KB), results go to `testing-log/`, traps to `TRAPS.md`, decisions to `DECISIONS.md`, tools to `TOOLS.md`; a card's acceptance is a scenario where it can be.

- **`nrscan` deleted** (Leonardo, 2026-10-24): the external scanner/poker of Nucleus games was replaced by the in-DLL console and the orchestrator; it stays in git history (`git show cbbffa2:crates/nrscan`). `nrcon` stays (console client; reaches several games with `--all` / `-i N`).
- **No winner's choice; the final fight is the whole of night 2; everyone sees the report** (Leonardo, 2026-10-24): winning ends the match with nothing offered (PROJECT_PLAN F13 dropped); the final arena is the fight between all players on night 2; winner and losers both get the game's report / Result screen and return to the Roundtable. Contraction and sudden death stay as designed in PROJECT_SCOPE §4 until Leonardo changes them. Verified on one PC 2026-10-24: after the ending, win and loss both go Result → Reward → Roundtable (testing-log).
- **After the first real match: the rule set changes** (Leonardo, 2026-10-24; cards RW-*, TASKS): (1) a player killed by another player/team or by the night-tide is **defeated for good** and becomes a spectator, who may keep watching or quit to the Roundtable and search again; (2) a defeated player drops their items (red phantom) on the ground for others to loot; **runes are discarded, not dropped** (Leonardo, 2026-11-01); (3) death by a mob or boss is the vanilla death: respawn with items, fetch the runes; (4) ~~Dormant Powers and weapons from bosses drop only for the team/player that landed the last hit~~ (superseded: RW-5 parked, orbs stay vanilla, 2026-10-31); (5) Day 1 and Night 1 run their normal length with **no night-tide**; the tide's vanilla behaviour starts on Day 2; (6) the match ends the moment one team/player is left alive, even in the first minutes; (7) a private match joined by password, outside matchmaking. Fixes: respawn far from all players; runes shared only inside the own team; no other player's map/compass position and nametag/lifebar only for the player you target; enemy count and strength use the vanilla Solo/Duo/Trio values. **Supersedes** "nights are elimination rounds / losers get up at the morning" (2026-10-22) and the M5-9 hold-until-morning for players killed by players; night 1 is no longer a round of its own.
- **Downed always, in Solo too** (Leonardo, 2026-10-24, after talking to a player; reverses "Solo: a kill is a real death" of 2026-10-23): no insta-death. A player at 0 HP is downed for the normal bleed-out (rival hits shorten it, rivals cannot revive); when it runs out the death is judged: a rival or the tide = defeated for good, anything else = the game's own respawn. Solo, Duo and Trio behave the same. Die-at-once (SpEffect 102130, flag 10007420) stays only as a one-PC test aid (offline solo has a revive effect).
- **Spectating = the mod's moved body, on by default** (Leonardo, 2026-10-25, Nucleus 3-player test): with `spectate 0` the game does nothing for a defeated player (no native spectator in our rivals-only match), so the held, invisible body follows a living player and the game's follow camera shows them. Works ("exactly like I wanted"), camera movement still to be smoothed (find the follow camera's target: `ChrExFollowCam` +0xA0 holds the followed position history; TASKS).
- **Map and compass show only your own team** (Leonardo, 2026-10-26; card RW-9): rival players and teams must never see each other's position on the world map or the compass bar, so the UI does not hand out where the enemies are and hunting stays a player's work. **Solo** = no other player on the map or compass. **Duo / Trio** (same rule) = a player sees only their own team's members there. Built 2026-10-27 for every team size (`game/hudfilter.rs`, PRs #21 / #22: a rival is on no map pin, compass mark, party bar or name tag, incl. the edge arrow; teammates stay); `game/maphide.rs` is the Solo fallback. Duo / Trio teammate runs are still untested. Teams otherwise behave as in vanilla (teammates keep their map / compass marks, name and bars). **Life bars stay vanilla** (Leonardo, 2026-10-26, withdrawing his stricter wording): the enemy bar a player gets by lock-on (R3), by hitting a player, or when that player's HP drops is kept as the game does it; nothing to hook. Name tag and party bar were hidden for rivals with the same hook (levers in MEMORY_MAP §20); open: the pin toast (TASKS RW-9).
- **Seamless Co-op will be replaced by our own networking** (Leonardo, 2026-10-30, while designing RW-5): do not build new cross-game features on SC's lobby, chat or flags. Features that need the other game's knowledge (RW-5: who landed the last hit) go behind the data channel (`net::bus` facts, NETWORKING §4); the game's own knowledge is the source on each side. Fail-open (vanilla behaviour) while a verdict is unknown.
- **RW-5 = first to loot, no last-hitter logic** (Leonardo, 2026-10-31): only the first team to OPEN a boss's Dormant Power screen may choose from it; the claim is made at open time and sent on the data channel (`ORB_CLAIM`); other teams' games lose the orb's interaction by nulling its `MapItemMan` slot. Supersedes the old "blank boss rows + verdict" plan (`rw5-boss-drops`). Skeleton merged (PR #26), off by default (next entry).
- **Boss orbs stay vanilla for now** (Leonardo, 2026-10-31, after the RW-5 skeleton): everyone can loot every Dormant Power orb, no claim, no last-hit rule. Reasons: the blank-the-pointer switch also stops a client claimer from picking (the client needs the host's copy), a second kill overwrites an unpicked orb's slot, and the real fix (interaction call-site hook) is an RE spike with crash risk. Rune lock for the killing team (RW-8) is the only boss-reward rule for now. The skeleton stays in the code, off (`orb_auto 1`), for a later discussion.

- **Match deaths: only a mob death respawns** (Leonardo, 2026-11-01): in a Battle Royale match a death by a mob, a boss or a fall is the game's own respawn; a death by a rival or by the night tide / zone is final (defeated for good, then spectate or the Roundtable). Nobody respawns after a rival kill. Falls count as mob deaths.
- **Runes of a defeated player are discarded** (Leonardo, 2026-11-01; RW-3): items drop where the victim fell, runes do not.
- **Solo night revive item: follow vanilla, test first** (Leonardo, 2026-11-01): `re/PROJECT_SCOPE.md` §1 wants a Solo kill at night to be a real death at once unless the player carries a revive item (Wending Grace, goods `0x400002BC`) that then works as in vanilla. Not decided as a rule and not built; it may already behave so. First step is a test card (TASKS "Solo night death"); the entry "Downed always, in Solo too" (2026-10-24) stands until then.
- **Next priorities and what waits** (Leonardo, 2026-11-01): firm next steps are Sites of Grace blocked during PvP combat and a first Duo / Trio test. A match time limit and the Solo die-at-once rule are open ideas, no schedule. Arena (MODE-02) comes after the first release; the code must be decoupled so a new mode is an assembly of existing modules.
- **Language** (Leonardo, 2026-11-01): code, comments and docs are English only.
- **Seamless Co-op's author is not contacted** (Leonardo, 2026-11-01): we never modify or redistribute SC, so there is nothing to ask. Docs no longer plan an "ask before release" step.
- **Red Phantom scrapped; scope synced with the game** (Leonardo, 2026-11-01): items just lie on the ground, no figure (simplicity first). What the build drops (weapons, talismans, consumables) is enough. The "sudden death" stage of the old scope is dropped as a term: respawns are already off, so the only open piece is a stall-breaker at the closed final circle (the vanilla tide damage today). Time limit: use the game's own limit if one exists, else define one (open, not built). `re/PROJECT_SCOPE.md` now ends with a "Scope vs game today" table that the agent keeps in sync.
- **The current build is the release candidate; core gameplay is frozen** (Leonardo, 2026-11-01, after a two-player Solo test with a friend): the Battle Royale core is exactly what we want to ship. Only three checks remain before Battle Royale is finished: (1) Sites of Grace blocked during PvP combat (TASKS GRACE-1), (2) Duo / Trio tested (DUOTRIO-1), (3) the vanilla revive item (Wending Grace) when a player is downed (SOLO-REVIVE, a test first). Everything else waits. Focus now: refactor and tooling (decoupled modules, better scenarios), then those three.
- **Refactor plan adopted** (Claude on Leonardo's brief, 2026-11-01; Opus 5.5 planned it, `REFACTOR_PLAN.md`): Battle Royale becomes one assembled list of modules (a `Ruleset`), `night_match` splits into ordered match rules, hook modules become thin adapters in the Feature list, each console command lives with its module. Two deliberate deviations from ARCHITECTURE: (1) `TeamCtl` and `CombatCtl` stay core services in the loop (every mode needs them); (2) config stays **one flat `nrpvp.toml`** with typed slices in code, instead of toml sections, because `nro`'s `[config]` keys, its forced config and preflight P7 depend on the flat keys. Behaviour of Battle Royale must not change; each PR proves it (unit/golden tests, named scenarios).
- **Two blockers come first: the match survives the host leaving, and more players** (Leonardo, 2026-11-02): top priority of the project, whatever the effort; supersedes "UI first" and the order in STATUS §3 until they are settled. **Host migration without any pause is the goal; a Call of Duty style pause (everyone reloads and comes back where they were) is the last resort.** Supersedes "accept that the match ends" (2026-10-21) and S10's no-go, which rested on static reading only. **Target: up to 12 players per match** (supersedes "every match has 6 players", 2026-10-19, once it works). Independence from Seamless Co-op stays the long-term wish; plan in HOST_MIGRATION.md.
- **Host migration first; player count later and only on good results** (Leonardo, 2026-11-02, same day, narrowing the entry above): 12 players is no longer a committed goal. Code lives in `nrpvp` on a branch, in a module behind a cargo feature outside the player build, not in a separate project (Claude's recommendation; HOST_MIGRATION §4).
- **NET-7 (sign-in without Bandai's server) released with guards for the lab stage** (Leonardo, 2026-11-02; resolves the hold of NETWORKING §7a): emulated Steam ids only, `Menu.IsEnableOnlineMode` false, instance exes blocked from the internet by a firewall rule Leonardo adds himself, nro's TCP watch must show no public endpoint. **Real Steam is a separate go/no-go** before any Friend Session on this path.
- **Use the game's own host migration** (Claude, 2026-11-03; HOST_MIGRATION.md): the exe promotes the new Steam lobby owner by itself, so no hook and no own migration unless FS-HM1 shows one is needed. Supersedes S10's no-go.
- **Priorities** (Leonardo, 2026-11-03): (1) GRACE-1, (2) finish the match start: MM-3g, MM-7, no Steam invite on the overlay, (3) killfeed. The sessions with friends are ongoing; he reports when he has the logs, so agents do not chase them.
- **Priorities changed the same day** (Leonardo, 2026-11-03): GRACE-1 done; the green beam of a rival's pin first, everything else frozen until it is gone (done, `mi-pin-beam`); then the killfeed, discussed with him before any build; the match start (MM-3g, MM-7) after that.
- **Killfeed design** (Claude, 2026-11-03; Leonardo handed the choice over: "something that reminds of Warzone / Fortnite", he may change it): game toasts at the bottom right, `downed` and `killed` lines, character names (not Steam names), own team blue, rivals red, the last rival who hit gets the kill, no shared kills, no count of who is left (Leonardo took it out). Why toasts and not blinking messages: they stack and fade like a feed and do not cover the fight (`re/TOAST_KILLFEED.md`).
- **Suite mode (N9) shelved** (Leonardo, 2026-11-02): one launch for many scenarios was built (branch `claude/nro-suite`, commit 0b5f9f4: console `reset` / `state` / `cfg`, `nro suite`, `scripts/suite.ps1`) but never run on the game and the estimated gain is small (about 20 to 12-15 min per full run, each scenario still loads its own expedition) against the risk of state leaking between scenarios. The headless rule twins (N6, `cargo test -p nrpvp sim_`) and `-Tag smoke` cover the speed need. Pick the branch up only if full runs become a bottleneck again.
- **GRACE-1 values** (Claude, 2026-11-03, Leonardo may change them): 15 s after the last hit (`grace_combat_secs`), a rival's HP falling within 30 m counts as my hit, and the lock ends early with no living rival within 100 m. Leveling up is blocked with the heal: the game's switch covers both. No message to the player for now.
- **Zero clicks after Commence, with fallbacks** (Claude, 2026-11-05; Leonardo's priority of 2026-11-03): the mod calls the game's own lobby job and ready mark (MM-3g, MM-7; `game/matchstart.rs`), no menu opened by the mod. A missing player no longer holds the match: after 60 s everyone in the lobby goes ahead if at least `mm_min_players` are here. If the game makes no lobby in 20 s the host gets the old Invite Members prompt. The joiner's way in is the link; the host's Steam invite (which shows a notification on the joiner's screen) is only a fallback after 25 s (`mm_join_auto = "link+invite"` in the player build). `mm_auto_start` stays off by default in the developer build so older scenarios keep their steps.
- **Launcher app (LAUNCH-1)** (Claude, 2026-11-06; Leonardo asked for a Riot / FFXIV-style launcher with an install wizard and left the stack open): **Tauri 2** (`crates/nrpvp-app`, `NightreignPvP.exe`): Rust for downloads, zip and Play, the UI as plain HTML/CSS/JS so the web draft's look carries over; no Node build step; one exe that needs WebView2 (built into Windows 10/11). Play reuses `nrpvp_launcher.exe`'s code (now the `nrpvp_launch` lib) and closes the app. Rejected: egui (styling it like a game launcher is slow work), Electron (size and RAM).
- **Release hosting: a public, binaries-only GitHub repository** (Claude, 2026-11-06; Leonardo agreed the same day and creates the repository himself. **Nothing is published there until the final test session with more than 3 players**; the launcher is a draft until then): the source repository is private, so its releases cannot be downloaded by players. `notgonnaleo/nightreign-pvp-releases` gets one release per version with `NightreignPvP.zip`, `NightreignPvP-Launcher.exe` and `release.json` (version, SHA-256 of each asset, pinned game exe and `nrsc.dll` hashes, patch notes). The launcher reads `releases/latest/download/release.json` (no API call, no rate limit). `scripts/publish-release.ps1`, RELEASE.md §5b.
- **Seamless Co-op is never in our release** (CLAUDE.md hard rule 3) and has **no GitHub release for Nightreign** (checked 2026-11-06: `yuiamoroll/NightreignSeamlessCoopRelease` holds only data files and `VERSION`; the mod is on Nexus, mods/3). The launcher installs from a GitHub release of that repository if one ever appears; until then it opens the Nexus page and installs the zip the player downloaded (found in Downloads by its content, or picked). An existing `nrsc_settings.ini` is kept.
- **No name tag on a locked-on rival** (Leonardo, 2026-11-06, from an in-game screenshot): a rival under lock-on showed its name twice, once over the game's own enemy life bar and once in the overhead name tag (class icon + name) that RW-9 let through for the lock-on target. Leonardo keeps the name over the bar, so the name tag site now hides every rival, the lock-on target included (`game/hudfilter.rs`). Supersedes "name tags only on the target" in RW-9.
- **Bots are a game feature again** (Leonardo, 2026-11-06; reverses 2026-10-24): AI bots fill a match to 6, for one-PC test matches and for players who want to play against bots; friends and bots in one match is required. Design by Claude in [BOTS.md](BOTS.md): the game's own bot manager (set 14, no player-list puppets in the player build), a host-side brain on top of the game's AI, solo first, friends + bots after the BOT-0 sync spike; entry recommended as a second card "Battle Royale (Bots)" (fallback: an "Allow bots" backfill setting in `nrpvp.toml` / launcher).
- **Bots: the entry is a Bots switch in Matchmaking Settings; no separate build** (Leonardo, 2026-11-06): the stock "No. of Password
  Players" row becomes Bots Off / On; Bots On + the Battle Royale card + Solo starts a match against 5 bots at once, in the normal build and
  zip. Bots start in separate places, never next to the player. A second expedition card was not possible (the game ignores new row ids).
  The `play-bots.ps1` / `dist-bots` path was removed. BOTS.md §2e.
- **Bots are a developer option for now; more aggressive** (Leonardo, 2026-11-07, after his first match): the Bots row shows only in
  the dev build (`menuflow::BOTS_MENU`); the player zip leaves the row as the game's own until bots are further along. Bots should come
  for the player: every minute the farthest living bot beyond 150 m is brought to a living enemy ~100 m from the player (BOTS.md §2b).
  A defeated player with only bots left loses at once. The Night's Tide fix (rain SpEffect) was his priority.
- **Bots for players again, as an offline training mode (beta)** (Leonardo, 2026-11-07, after his second bots match, which he liked;
  supersedes "a developer option for now" of the same day): the Bots row is on in the player build; its help says it is an offline
  training mode, Solo against 5 AI bots with no other players. Bots Off stays the online Battle Royale queue.
- **K/D scoreboard in the match** (Leonardo, 2026-11-07): own numbers only, shown as the game's blinking centre message when the pause
  menu opens (Esc / pad Start); mob and fall deaths count as deaths. Not a separate key (no free pad button), not the key-guide text
  swap (tried, reverted). The professional version is a GFx edit of the game's menus with JPEXS, later (SCOREBOARD §8b).
- **No match timer: the Night's Tide closes as far as possible** (Leonardo, 2026-11-07, after looking at how Fortnite, Apex and
  Warzone end a match: none uses a fixed timer, the zone closes almost to nothing and that forces the end). The final circle of
  Night 2 closes to a point (TIDE-MAX, `gamedata/playarea-rows.json`); the tide's damage stays vanilla (he finds it balanced).
  "Most kills wins, then most HP" is only a tiebreak for edge cases (simultaneous deaths, a stalemate cap) and is not built.
  Replaces PROJECT_SCOPE §1 "the final circle keeps the game's size (40 m)".
- **Matchmaking is finished** (Leonardo, 2026-11-07, after the 3-player friend session): the zero-click flow (Commence, nothing else,
  no Steam window, no Steam notification) is how it was wanted from the start; no more changes planned. MM-8 (host by best connection)
  and the 3-team Duo match stay ideas, not work.
- **F10 while defeated also leaves the session** (Leonardo, 2026-11-07): today a host who presses F10 goes to the Roundtable but stays
  in the session (and stays host) while the others play; leaving should take the player out of the session at once (TASKS FS7-8).
- **A downed rival's near-death gauge stays hidden** (Leonardo on Claude's advice, 2026-11-07). Report (friend session 2026-11-07):
  in Solo a downed rival showed no gauge. RCA: the RW-9 HUD filter (`game/hudfilter.rs`, since 2026-10-27) hides every rival's party bar
  and name tag so their position does not show through walls; the gauge over a downed character is drawn by that same name-tag element
  `[HYPOTHESIS: not yet seen with the filter off; check: two instances, a downed rival, `rivalhud 0` → the gauge shows]`. The removal of
  the locked-on target's name tag (2026-11-06, it clashed with the HP bar and buffs) only affects the one target, so it is not the cause
  for the others. Why keep it: as in Apex / Warzone the downed pose is the signal; the bleed-out time is a teammate's tool (revive or
  not), and a tag through walls would mark the downed player's spot. **A teammate's gauge must still show** (teammates are not
  filtered; checked in DUOTRIO-1).
- **Launcher release path: the public releases-only repository; publishing frozen** (Leonardo, 2026-11-08): a private repository with a token in the launcher was weighed and dropped (the token can be pulled out of the exe, and the installed DLL can be copied anyway). Later, when Leonardo's server and R2 bucket exist, `release.json` and the zip can move there (the launcher only reads one address). Nothing is published until Leonardo says so; PR #68 waits, the next friend session gets the zip by hand. Server sizing: a 2 GB Lightsail is enough if no file passes through the server (downloads and logs straight to the bucket by short-lived links; `release.json` public on the bucket; log upload switchable; a stateless API).
- **Duo / Trio fill short parties with random players** (Leonardo, 2026-11-08; reverses "complete parties only" of 2026-10-19 in MM-6): a player alone, or a pair in Trio, is queued and gets random teammates, as players expect from a battle royale. Needed before the first release. Card MM-FILL (TASKS); it also closes the MM-6 gap that the match does not take its teams from the queue roster.
- **Arena (MODE-02) development starts now** (Leonardo, 2026-11-08): before the Battle Royale release (which waits only for a match
  with more than 3 players) and before the launcher. Reverses "Arena comes after the first release" (2026-11-01). Rules decided the
  same day (PROJECT_SCOPE §Arena 1): a card right below Battle Royale, Solo / Duo / Trio = 1v1 / 2v2 / 3v3, an offline bot at least
  in 1v1, always the Nightlord's arena, spawns facing each other at medium / long range, loadout taken from the Sparring Grounds'
  Sparring Equipment (none = the Nightfarer's default weapon at legendary tier), every weapon at max upgrade, everyone level 15,
  **best of 3** (replaces "win three rounds" of the first draft).
- **Arena details** (Leonardo, 2026-11-08, PROJECT_SCOPE §Arena 1): **Heolstor's** arena, no boss; between rounds everything resets
  **except the ultimate gauge**, which keeps charging across rounds; **3-minute rounds** (tuned in testing); **relics ON** (trying relic
  + weapon + talisman builds is the point; the agent had proposed off); **maximum flask count** for everyone; the loadout is the
  **whole inventory** from the Sparring Grounds (a first slice may bring only the equipped items); loser sees defeat, winner VICTORY,
  then both return to the Roundtable out of the session.
  **Ultimate only in the last round** (Leonardo, 2026-11-08, after ARENA-2's fights, where the gauge never charged in the arena):
  rounds 1 and 2 have no ultimate (the gauge held empty), round 3 starts with it full. Replaces "the gauge keeps charging across
  rounds" above. Agent's note: simpler than charging it, and it makes the deciding round special; a 2-0 match never sees an ultimate.
  **Superseded 2026-11-17, "One ultimate per round" below.**
  **Session after an Arena match** (Leonardo, 2026-11-08): random players leave the session when they return to the Roundtable;
  players who are Steam friends or came in through a direct invite stay together. Today nobody leaves: the game's SC lobby outlives
  the match (MATCHMAKING "Old SC lobby still around"), not an emulation artifact. Built with spike 6 / ARENA-2.
  Checked after the fact (agent, 2026-11-08): Heolstor's `m18` is also the simplest Nightlord arena to use: `m19` is shared by the
  other Nightlords (3× the event script), `m16*` are DLC arenas, `m12` is the Shelter. Open risk is its size (~300 m across, large
  for a 1v1): answer with our own boundary if a real fight shows kiting, not another map (the warp target is one line of data).

## 2026-11-10 — The Arena bot stays at level 13 with the hero's stock weapon (Leonardo)
After his first match against the bot (ARENA-4) Leonardo first asked for a level-15 bot with a legendary weapon, then kept it as it is:
"ta bom assim". The bot uses the game's highest bot table (`NPCBotTableParam` 30000, soul level 13, the hero's `CharaInitParam` kit).
If it is reopened: a table of our own in `regulation.bin` (copies of `CharaInitParam` 30000..30900 at level 15, max-upgrade weapon).

## 2026-11-10 — A won Arena match may count as an Everdark win: accepted (Leonardo)
A won Arena match counts as an Everdark win for the game (seen: the vanilla Sovereign Sigils tutorial at the table afterwards,
testing-log ARENA-4 runs 1794280877..1794281105). Leonardo finds the Everdark reward fair for a won match ("acho justo"), so it stays
for the open release. Seamless Co-op keeps its own save (`.co2`), the vanilla `.sl2` is not touched. What else the win marks
(rewards, journal, win counts) is `[HYPOTHESIS]`: only the tutorial was seen. If it is reopened: keep the Arena WIN from setting the
game's clear flags.

## 2026-11-11 — Cloud: logs first, match data later, accounts optional and later; server in C# (Leonardo)
Leonardo asked whether a database for player logs, matches and metrics is worth it. Decided: a **Send logs** button in the launcher
first (before the open release), match summaries after the release, **optional** accounts later with Steam sign-in and a Discord
link, no passwords of our own, never needed to play. No rankings (no anti-cheat, results come from the clients). The server is C#
(ASP.NET Core) on Leonardo's Lightsail with his Postgres; log files on disk, never in Postgres; no personal data by default (a random
install id; Steam IDs in logs hashed by the launcher). Design: [CLOUD.md](CLOUD.md).

## 2026-11-13 — Player releases live in their own public repository; the launcher needs no token (Leonardo; releases part superseded 2026-11-23)

Releases moved from `aspherya/nightreign-pvp` to `aspherya/nightreign-pvp-releases`: a release in the source repository ships
GitHub's source archives, and a token that can read its releases can read its source. The releases repository is public, so the
launcher reads it without a token and nothing secret is in the exe. A read-only token built into the launcher was implemented
first and dropped once the repository went public: anyone with the exe could have recovered it, and it would have expired on
every player. Only CI holds a token (`RELEASES_PUBLISH_TOKEN`, write to the releases repository only). How-to: RELEASE.md §6.

## 2026-11-13 — Battle Royale and Arena always ship together; CI builds the game data (Leonardo)

No more `arena` cargo feature and no separate BR-only game-data package (`gamedata-player`): every build, dev or player, has both
modes and the saved-loadouts menu. The release workflow builds `dist/gamedata` itself from pinned inputs (original game files +
WitchyBND, ESDLang, Smithbox DLLs, me3; `scripts/prepare-ci-inputs.ps1`) instead of a prebuilt BR-only package, so a change to
`gamedata/` or `tools/loadouts/` reaches players without a manual rebuild. Why: the split kept the Arena and saved loadouts out of
every release (v0.1.28 had the code compiled out). How-to: RELEASE.md §6.

## 2026-11-13 — Merge commits only; release versions count up from the workspace version (Leonardo)

The source repository allows only merge commits (squash and rebase merging are off in the repository settings; branch rules
need a paid plan on a private repository). Why: the squash of the dev → main PR #7 made the next dev → main PR conflict on
every shared file. Release versions: the workspace version (`Cargo.toml`, now 0.3.0) or one patch above the highest tag in the
releases repository, drafts included; no run number. Why: the new releases repository started at 0.1.x, below the 0.2.0 testers
had installed, and the launcher refuses downgrades. How-to: RELEASE.md §6.

## 2026-11-17 — An Arena round ends at 1 HP, with no down (Leonardo)
For the whole Arena match each game holds its player immortal (Set Character Immortality 2004:12, flag 10007520, event 99100920):
a real hit stops at 1 HP and that ends the round (`arenarounds::BEATEN_HP`). Nobody is downed, so no get-up is needed. Why: the
get-up after a down killed the loser early in the next round on real Steam twice (2026-11-15, 2026-11-17, 0.3.0). The bar get-up of
2026-11-16 stays only as the fallback should a down still happen.

## 2026-11-17 — Arena: one ultimate per round, consumables back every round (Leonardo)
- **One ultimate per round:** every round starts with the ultimate gauge full; once used, it cannot charge again in that round
  (hits that would charge it are taken back). Replaces "Ultimate only in the last round" (2026-11-08). Built in
  `game/arenarounds.rs` (`UltOnce`): an ultimate whose gauge drains while it plays is never cut short, the gauge is only kept from rising.
- **Consumables reset every round:** each round reset gives back the pouch consumables captured at the Roundtable
  (`arenaloadout::refill_goods`), as the flasks already were.
- **Ultimates must hurt the rival like any enemy:** some ultimates deal no damage to a rival player (Leonardo, 2026-11-17). Open:
  which ones, and why (TASKS ARENA-ULT-DMG).

## 2026-11-17 — Arena timeout: overtime with an HP drain, then HP % (Leonardo)
At the 3-minute limit nobody wins on the spot: for up to 60 s (`arenarounds::OVERTIME_MS`) each game drains its own player's HP,
2 % of the maximum per second (the bot's too in a bot match), and the first to fall to 1 HP loses the round through the 1 HP rule.
In overtime only the host judges and sends the verdict (both at 1 HP in its view = a draw, played again); nobody at 1 HP at the end
of the overtime = the old HP % judgement. A "One minute left!" message comes 60 s before the limit and the rune counter shows the
seconds left (ARENA-CLOCK). Why: the HP % rule alone rewarded running away after a first hit in a large arena, and flasks skewed it;
the drain forces a fight, keeps a comeback possible and needs no new lag-sensitive judgement. Options weighed: HP % (kept as the
fallback), sudden death (host advantage under lag), replay-only draws (endless).

## 2026-11-17 — The launcher refuses an old mod, and refuses to start without the version check (Leonardo)
`nrpvp_launcher.exe` and the launcher app's Play do not start the game when the installed mod (`VERSION.txt`'s `version` line, else
the app's `release.json`) is older than the newest published release, or has no version at all. **When the newest version cannot be
learnt** (offline, GitHub's 60-per-hour limit, an error, 10 s per request) **the game does not start either**: the player is told to
connect and try again, and `launcher.log` says why. Why: everyone in a match needs the same version, and the old "Play without
updating" link let players into mismatched matches. Claude first proposed starting anyway when GitHub is unreachable; Leonardo
chose to block ("simply does not let the game open"). Accepted cost: a GitHub outage, or a shared IP over the 60-per-hour limit,
stops every player until it passes. Zips built locally without `-Version` say `version local` and are never checked, so
friend-test zips keep working; `NRPVP_SKIP_VERSION_CHECK=1` is the developer escape. How-to: RELEASE.md §5c.

## 2026-11-23 — Releases go back into the source repository, now public at notgonnaleo/nightreign-pvp (Leonardo)

Leonardo transferred the repository from `aspherya` to his account `notgonnaleo` and made it public, and asked that releases be
published in it instead of `aspherya/nightreign-pvp-releases`. The 2026-11-13 reasons for a separate repository (source archives on
every release, a token that reads releases reads source) no longer apply to a public repository. CI now publishes with the built-in
`GITHUB_TOKEN` and tags the source commit; the build-inputs draft stays a draft (drafts are invisible to the public). The workspace
version went to 0.4.0 so the first release here is above v0.3.3. **Bridge:** launchers up to v0.3.3 have the old repository built
in, so every release is also mirrored there (`LEGACY_RELEASES_REPO`, needs `RELEASES_PUBLISH_TOKEN`); their auto-update installs a
launcher that reads the new one. Drop the mirror once no player runs such a launcher. How-to: RELEASE.md §5b, §6.
