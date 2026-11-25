//! T-008: `nrpvp.toml` next to the DLL. Created with defaults if missing; the effective values are logged.
//! A file that fails to parse never stops the mod: the defaults are used and the error is logged.
//!
//! R5 (2026-10-20) removed the overlay, hotkey, probe, C1-prototype and downed-experiment settings with their features; a file
//! that still has them loads fine (one warning line names the ignored keys).

use std::path::Path;

use serde::Deserialize;

pub(crate) const FILE_NAME: &str = "nrpvp.toml";

/// Hostile-to-player candidates from TECHNICAL_NOTES.md section 4 (Smithbox NR TEAM_TYPE enum; HYPOTHESIS
/// until TC-004): 6 Enemy, 29 Indiscriminate, 3 Black Ghost, 16 and 13 Invader, 2 White Ghost, 1 Player.
pub(crate) const DEFAULT_CANDIDATES: [u8; 7] = [6, 29, 3, 16, 13, 2, 1];
pub(crate) const DEFAULT_PERSIST_MS: u64 = 50;
pub(crate) const DEFAULT_WATCH_MS: u64 = 1000;
/// T-020: team_type the PvPvE mode writes over the expedition default (2).
pub(crate) const DEFAULT_PVPVE_TEAM: u8 = 29;
pub(crate) const DEFAULT_PVPVE_INTERVAL_MS: u64 = 250;
const PVPVE_INTERVAL_MS_RANGE: (u64, u64) = (50, 5000);
pub(crate) const DEFAULT_MM_MATCH_PLAYERS: u64 = 2;
const MM_MATCH_PLAYERS_RANGE: (u64, u64) = (2, 6);
/// MM-4: queue lobby size, the smallest match, and the wait for more players once the lobby has a 2nd member.
pub(crate) const DEFAULT_MM_MAX_PLAYERS: u64 = 6;
pub(crate) const DEFAULT_MM_MIN_PLAYERS: u64 = 2;
pub(crate) const DEFAULT_MM_FILL_WAIT_S: u64 = 90;
const MM_FILL_WAIT_RANGE: (u64, u64) = (5, 600);

/// Sanity bounds so a typo cannot spin a thread or silence the watcher.
const PERSIST_MS_RANGE: (u64, u64) = (10, 5000);
const WATCH_MS_RANGE: (u64, u64) = (100, 60_000);

/// Contents written when the file does not exist.
pub(crate) const DEFAULT_FILE_TEXT: &str = "\
# nrpvp configuration. Edit and restart the game; the effective values are logged at startup. Keys that no longer exist are
# ignored (one warning line names them).

# team_type values (u8) the console's team_set accepts for the local player.
team_candidates = [6, 29, 3, 16, 13, 2, 1]

# Persist mode: re-apply the selected value this often (milliseconds, 10..5000).
persist_interval_ms = 50

# How often the watcher logs PlayerIns changes (milliseconds, 100..60000).
watch_interval_ms = 1000

# T-030: false (default) = player build: PvP always on, menus and messages are the game's own UI.
# true = developer mode (the console for nrcon.exe, pause/resume, the dev switches below). Only a developer DLL honours it.
dev_mode = false

# T-020 PvPvE mode: while on, this client rewrites its own copy of team_type for EVERY player whose value is 2 (the
# expedition default) to pvpve_team, every pvpve_interval_ms (50..5000). Other values (e.g. 1 in the Roundtable) are
# never touched. Console `pvp 0` pauses the mode (and restores captured originals), `pvp 1` resumes it.
# Every client in the session must run it: hostility is decided from each client's own copies.
# pvpve = true   (dev mode only; the player build always has PvP on)
pvpve_team = 29
pvpve_interval_ms = 250
# T-026 scheme. \"rivals\" (default): this client writes pvpve_team only to players that are RIVALS of the local player
# (per the rosters below); itself and its allies stay on the vanilla value, so graces, pickups and revives keep working.
# \"all\": every player with value 2 is rewritten (the T-020 behaviour). Anything else: warning + \"rivals\".
pvpve_scheme = \"rivals\"
# Players per team (1..=3). 1 = free-for-all. Rosters come from the per-player id (ChrIns+0x78), sorted, chunked by team_size.
team_size = 1
# Deprecated alias (FS-0): auto_team_all = N means pvpve = true, pvpve_team = N when pvpve is not set. 0 = off.
auto_team_all = 0

# MM-1 Steam access probe (dev mode only, read/listen-only): waits for the game's own Steam init, logs which flat Steam interfaces are
# reachable and every lobby event (with owner, members and lobby data) under the prefix \"mm_probe:\". Registers listen-only callbacks;
# never calls Steam setters. The player build forces this off.
mm_probe = false

# NET-1/2: the mod-to-mod data channel over Steam networking messages (NETWORKING.md). Its peers are the other members of your
# Seamless Co-op lobby; extra peers for tests via console `net_peer`. In the player build it follows `matchmaking`.
net = true

# Several game instances on one PC (orchestrator, dev mode only): the game closes its own single-instance mutex (SekiroMutex) so
# the next instance can start. Also the console command `mutex`.
allow_multi_instance = false

# MM-2 public matchmaking queue (dev mode only; the player build forces it off). false = nothing of it runs. true = a queue lobby
# worker on the Steam thread, driven from the console (mm_queue_solo, mm_leave, mm_status) and from the table menu (br_menu).
# Needs Seamless Co-op (nrsc.dll) loaded and no EasyAntiCheat module; otherwise queuing is refused and the reason is shown.
# mm_match_players = players per match (2..=6); a team size from the queue mode (solo 1 / duo 2 / squad 3) overrides team_size
# for that match only. Log lines start with \"mm_queue:\".
mm_queue = false
# mm_match_players = 2

# MM-4 Solo Battle Royale matchmaking, the player setting (player build only; developer mode uses the mm_* switches). true (default)
# = Solo Battle Royale from the table menu (Number of Players = Solo, Commence): it finds other players and gets them into the
# host's Seamless Co-op lobby; the host starts the expedition. false = no matchmaking at all. Needs Seamless Co-op.
matchmaking = true
# MM-4 lobby rules (queue lobby size 2..=6; the match starts when it is full, or mm_fill_wait_s seconds (5..=600) after the lobby got
# its 2nd member if it holds at least mm_min_players). Everybody who should meet needs the same mm_max_players. Setting
# mm_match_players (the older switch above) makes min = max = that number.
mm_max_players = 6
mm_min_players = 2
mm_fill_wait_s = 90

# MM-3 (dev mode only; needs mm_queue = true; the player build forces it off): after MATCH READY, get every roster member into the host's
# Seamless Co-op lobby. false = exactly MM-2. mm_join_auto = what happens by itself: \"invite\" (the host's mod invites every absent
# player; default) | \"invite+link\" (plus the joiner's mod opens a steam://joinlobby link) | \"link\" | \"link+invite\" (the link, and the invite only for a player it did not bring in) | \"none\" (only the console's mm_join).
# Log lines start with \"mm_join:\".
mm_join = false
mm_join_auto = \"invite\"
# MM-3e (dev mode only; needs mm_join = true; the player build forces it off): the host's mod creates Seamless Co-op's lobby itself at
# MATCH READY (CreateLobby(Public, 6), the call the game makes for Invite Members) instead of asking the host to open Invite Members.
# If Seamless Co-op does not adopt it within 10 s the mod leaves it and the host is asked to open Invite Members. Log lines start
# with \"mm_host_create:\".
mm_host_create = false
# MM-3f (dev mode only; needs mm_join = true; the player build forces it off): while this player is the host at MATCH READY and no Seamless
# Co-op lobby exists yet, the Steam invite window that Matchmaking Settings > Invite Members opens is skipped (hardware execute breakpoint on
# ISteamFriends::ActivateGameOverlayInviteDialog, no code is patched; only calls from nightreign.exe are skipped). The game still makes its
# own lobby; the mod sends the invite. Everywhere else the window works as normal. The console's mm_quiet_test tries it solo.
# Uses debug registers, so it is switched off (with a warning) when mm_lobby_trace = true.
# Log lines start with \"mm_quiet_invite:\".
mm_quiet_invite = false
# MM-3g + MM-7 (dev mode only; needs mm_join = true; the player build turns it on with matchmaking): no click after Commence. At MATCH READY the
# host's game makes its lobby by itself, and once everyone is here every player is marked ready, so the expedition starts by itself. Logs \"matchstart:\".
mm_auto_start = false

# MM-3c (dev mode only; the player build and the mmtest zip force it off): find what creates Seamless Co-op's lobby A. Arms hardware
# execute breakpoints (debug registers, no code is patched) on ISteamMatchmaking::CreateLobby and
# ISteamFriends::ActivateGameOverlayInviteDialog on every thread and logs each hit (3 per address, 10 min) with a call stack as
# module+offset frames. Log lines start with \"mm_lobby_trace:\".
mm_lobby_trace = false

# UI-0 (dev mode only; the player build forces it off): log every text id the game looks up (menus, dialogs, talk), once each, as
# \"ui_text_trace: TEXT <FMG> <category> id <id>\". Hardware execute breakpoints on the game's text lookup (no code is patched); runs
# 20 minutes from launch. Menus may stutter while it runs. Uses the debug registers, so mm_quiet_invite and mm_lobby_trace are switched
# off while it is on.
ui_text_trace = false

# UI-1 step 0 (dev mode only; the player build forces it off): log every event flag that changes (all flag groups with storage, every
# 50 ms) as \"ui_flag_watch: flag <id>: off -> ON\". Read-only. Flags that flip more than 20 times in 10 s are muted.
ui_flag_watch = false

# UI-5 research (dev mode only; the player build forces it off): log who asks the game for the text of a toast (\"Ally is showing
# location\" and its relatives, text ids 337700..337799) as \"toast_trace:\" with the stack's code addresses. One hardware breakpoint
# (the Commence slot), so it excludes br_menu.
toast_trace = false

# UI-1 A/B/C (dev mode only; the player build forces it off): hardware breakpoints on the stock expedition menu (Commence checks,
# Number of Players list, Cancel matchmaking) and on the text lookup for the matchmaking ids; logs registers, stack return candidates
# and changes of the menu's settings object as \"ui_menu_trace:\". No code is patched. Turns ui_text_trace, mm_quiet_invite and
# mm_lobby_trace off (debug registers).
ui_menu_trace = false

# OFF-1 game mode: \"br\" (default) = Battle Royale: Commence in the stock expedition menu starts the matchmaking queue (br_menu).
# \"offline\" = Commence with Solo starts the game's own solo expedition at once (no queue, no second player), for solo play and
# testing; everything else (PvP rules, game-data package, hawk crossing) is the same. Logs \"mode: <mode>\" at start and on every
# change. The developer console switches it live: mode br | mode offline.
mode = \"br\"

# UI-1 option 1 (dev mode: needs mm_queue = true; the player build ignores this line and turns it on with matchmaking = true): Battle Royale from the stock
# expedition menu. Commence Expedition with Number of Players = one player starts the Solo queue and the game's own dialog says
# so; Commence again stops searching; once the match is ready Commence starts the expedition as normal. Duo/Trio say \"coming
# soon\". Hardware breakpoint (shared slot 2, works next to mm_quiet_invite), no code patched. Logs \"br_menu:\".
br_menu = false

# UI-2 spike (dev mode only; the player build forces it off): 25 s after a world is loaded, set the three test flags of
# gamedata/emevd-events.json in turn (10 s apart, twice) so our Roundtable events show their test messages (dialog / blinking /
# status). Needs the game-data package (default launch profile). Logs \"ui_msg_test:\".
ui_msg_test = false

# UI-2: matchmaking status as the game's own on-screen messages (blinking text: searching N of M, match found, host: Invite Members,
# everyone is here, stopped). Needs the game-data package (default launch profile / player zip) and the queue (mm_queue or, in the
# player build, matchmaking = true). Logs \"ui_msg:\".
ui_messages = true

# MM-3d (dev mode only; needs mm_queue = true; the player build and the mmtest zip force it off): enables the console's
# mm_sc_create. It calls ISteamMatchmaking::CreateLobby(type, 6) like the game does for Invite
# Members, watches that lobby for 20 s and logs whether Seamless Co-op adopted it (nrsc_lobby_type appears) or not (then the mod
# leaves it). It writes nothing to the lobby. Log lines start with \"mm_sc_create:\".
# mm_sc_create_type = \"public\" (default) | \"friends\" | \"private\".
mm_sc_create = false
mm_sc_create_type = \"public\"

# T-032 snap-back test (dev mode only, LOCAL player only): position candidates, as PlayerIns+0xOFF, [PlayerIns+0xPTR]+0xOFF or
# Physics+0xOFF (inside the player's physics module) (3 x f32 each). Console pos_next selects one, pos_save saves the local
# player's values of ALL candidates, pos_go writes the saved 12 bytes into the SELECTED candidate only and logs every candidate
# after 100 ms, 500 ms and 2 s.
# Invalid entries are skipped with a warning.
pos_candidates = [\"Physics+0x70\", \"PlayerIns+0x6C0\", \"PlayerIns+0x6D0\", \"[PlayerIns+0x60]+0x1EC\", \"[PlayerIns+0x6B0]+0x17C\", \"[PlayerIns+0x6B0]+0x18C\", \"[PlayerIns+0x5F0]+0x64\"]

# T-035 rival revive rule (dev mode only; the player build always uses \"redown\"): when a RIVAL completes the revive meter of
# the LOCAL player, the player is downed again at once (HP written to 0) instead of being revived; allies revive as in vanilla.
# \"vanilla\" turns the rule off (dev mode only), for comparison tests.
rival_revive = \"redown\"

# T-039 downed rule (both modes, always on): while the LOCAL player is downed, a hit by a RIVAL never advances the revive bar and
# takes this many seconds (0.5..15) off the bleed-out clock. Hits by allies / yourself / the environment are left alone.
rival_hit_seconds = 3.0

# RW-4 free first day (both modes): Day 1 and Night 1 run this many real seconds (120..=3600) with no Night's Tide and no night;
# the host then starts Day 2, where the tide and the night behave as in vanilla. 0 = the vanilla Day 1 with its tide and night.
day1_secs = 1020

# BOT-1 (BOTS.md): in an offline expedition (mode = \"offline\"), this many hostile AI Nightfarers (0..=5) join at the landing and
# fight you, each other and the monsters. 0 = none.
bots_fill = 0

# RW-1 defeat: a player killed by another player or by the tide stays dead for the rest of the match. This many seconds (0..=600)
# after the defeat the game shows the defeat screen and returns the player to the Roundtable (to search for another match).
# 0 = stay and watch until the match ends; press F10 to leave at any time.
defeat_exit_secs = 0

# GRACE-1: after a PvP fight (a rival hit you, or you hit a rival) Sites of Grace give nothing for this many seconds (0..=120), or until
# no rival is left within 100 m (the rival died or ran off). 0 = the rule is off and graces always work.
grace_combat_secs = 15

# RW-1 spectating: a defeated player's camera follows a player who is still in the match (the invisible dead body is moved to
# the watched player every tick). false = the dead body stays where it fell.
spectate = true

# RW-6 private match (both modes): players who set the same code (up to 32 letters, digits, - and _) meet each other in a private
# match, outside the public matchmaking. Empty = the public queue. Case does not matter.
private_code = \"\"

# F3 separate team spawns (both modes, off unless set): when the hawk drops you at the start of an expedition, the mod moves your
# own character spawn_radius metres (20..1000) from the drop point, each team in its own direction, 40 m higher; you then land
# as usual. Teams come from the player ids, so every game agrees. Log lines start with \"T-028 SPAWN\".
team_spawns = false
spawn_radius = 200.0

# UW-1 ultrawide (both modes, on by default): no black bars on a screen that is not 16:9 (21:9, 32:9, 16:10). The game then draws
# the full window; on a 16:9 screen nothing changes. false = the game's own 16:9 picture with bars. Log lines start with \"ultrawide:\".
ultrawide = true

# T-018: absolute directory for the logs (default = next to the DLL). Logs go to <log_dir>\\<DLL folder name>\\nrpvp.log
# and ...\\logs\\. Use a TOML single-quoted string so backslashes stay literal. Nucleus deletes its Instance<N> folders
# on close; point this somewhere permanent. Falls back to the DLL folder if unusable. Read before logging starts.
# log_dir = 'C:\\some\\folder'

# HAWK-2 hawk crossing (both modes, on by default): every start ride flies the one crossing line (the route file in the game-data
# package). The stick does nothing while you cruise; hold it in any direction to dive (dive_hold, 50..=5000 units: the game counts
# about 1600 per second of holding), then steer for dive_secs seconds (1..=15) before the hawk drops you. Leave the hawk any
# time with the vanilla Spectral Hawk Tree buttons (hold action, or L3). dive_boost = the hawk's x1.3 speed while diving;
# start_cut = skip the hawk's grab animation at the ride start. enabled = false gives the vanilla start ride.
# Keep this table last: every key below a [table] line belongs to that table.
[hawk]
enabled = true
dive_hold = 400
dive_secs = 7.0
dive_boost = true
start_cut = true
";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    /// T-030: developer mode. false (default) = player build: PvPvE forced on with the rivals scheme, the dev switches off,
    /// no console. Only a developer DLL (`dev` feature) honours `true`.
    pub dev_mode: bool,
    pub team_candidates: Vec<u8>,
    pub persist_interval_ms: u64,
    pub watch_interval_ms: u64,
    /// T-020: PvPvE mode on (after the alias is resolved in `parse`).
    pub pvpve: bool,
    pub pvpve_team: u8,
    pub pvpve_interval_ms: u64,
    /// T-026: `"rivals"` or `"all"` (normalised in `parse`; read it through [`Config::scheme`]).
    pub pvpve_scheme: String,
    /// T-026: players per team, clamped to 1..=3 in `parse`.
    pub team_size: u64,
    /// BOT-1: hostile bots that fill an offline expedition (0..=5; 0 = none). BOTS.md.
    pub bots_fill: u64,
    /// Deprecated FS-0 alias of `pvpve` + `pvpve_team`.
    pub auto_team_all: u32,
    /// MM-1: Steam access probe on (developer mode only; forced off in the player build).
    pub mm_probe: bool,
    /// NET-1/2: mod-to-mod data channel on (developer builds: the file; player build: follows `matchmaking`).
    pub net: bool,
    /// Dev mode only: close the engine's single-instance mutex in this process (game/multi_instance.rs).
    pub allow_multi_instance: bool,
    /// MM-2: public matchmaking queue on (developer mode only; forced off in the player build).
    pub mm_queue: bool,
    /// MM-2: players per match, clamped to 2..=6 in `parse`.
    pub mm_match_players: u64,
    /// MM-4: queue lobby size (2..=6); the match starts when the lobby is full. `mm_match_players` in the file overrides min and max.
    pub mm_max_players: u64,
    /// MM-4: the smallest match (2..=`mm_max_players`).
    pub mm_min_players: u64,
    /// MM-4: seconds after the lobby got its 2nd member before it may start with `mm_min_players` or more (5..=600).
    pub mm_fill_wait_s: u64,
    /// MM-4: player setting. true (default) = the player build runs Solo matchmaking (queue, join step, quiet invite). No effect in developer mode.
    pub matchmaking: bool,
    /// MM-3: join the host's Seamless Co-op lobby after MATCH READY (developer mode + `mm_queue` only; forced off in the player build).
    pub mm_join: bool,
    /// MM-3: `"invite"`, `"invite+link"`, `"link"` or `"none"` (normalised in `parse`; read it through [`Config::join_auto`]).
    pub mm_join_auto: String,
    /// MM-3e: the host's mod creates the Seamless Co-op lobby at MATCH READY (developer mode + `mm_join` only; forced off in the player build).
    pub mm_host_create: bool,
    /// MM-3f: skip the Steam invite window while this player is the host at MATCH READY without a Seamless Co-op lobby (developer mode + `mm_join` only; forced off in the player build and when `mm_lobby_trace` is on).
    pub mm_quiet_invite: bool,
    /// MM-3g + MM-7: no click after Commence (developer mode + `mm_join`; the player build turns it on with `matchmaking`).
    pub mm_auto_start: bool,
    /// MM-3c: lobby-creation call-stack tracer (developer mode only; forced off in the player build).
    pub mm_lobby_trace: bool,
    /// UI-0: text-id tracer (developer mode only; forced off in the player build; turns `mm_quiet_invite` and `mm_lobby_trace` off).
    pub ui_text_trace: bool,
    /// UI-1 step 0: fast all-groups flag-change logger (developer mode only; forced off in the player build).
    pub ui_flag_watch: bool,
    /// UI-5 research: who asks for a toast's text (developer mode only; forced off in the player build; takes the Commence debug register).
    pub toast_trace: bool,
    /// UI-1 A/B/C: expedition-menu tracer (developer mode only; forced off in the player build; takes the debug registers).
    pub ui_menu_trace: bool,
    /// UI-1 option 1: Commence Expedition drives the Battle Royale queue (developer mode + `mm_queue`; the player build turns it on with `matchmaking`;
    /// off while a dev tool that owns all debug registers is on).
    pub br_menu: bool,
    /// OFF-1: `"br"` (default) or `"offline"` (normalised in `parse`; read it through [`Config::game_mode`]).
    pub mode: String,
    /// UI-2 spike: set the test message flags in turn (developer mode only; forced off in the player build).
    pub ui_msg_test: bool,
    /// UI-2: matchmaking status as on-screen game messages (default true; needs the queue and the game-data package).
    pub ui_messages: bool,
    /// MM-3d: the "create SC lobby (test)" dev button (developer mode + `mm_queue` only; forced off in the player build).
    pub mm_sc_create: bool,
    /// MM-3d: `"public"`, `"friends"` or `"private"` (normalised in `parse`; read it through [`Config::sc_create_type`]).
    pub mm_sc_create_type: String,
    /// T-032 (dev only): snap-back position candidates, `PlayerIns+0xOFF` or `[PlayerIns+0xPTR]+0xOFF`.
    pub pos_candidates: Vec<String>,
    /// T-035: `"redown"` (default) or `"vanilla"` (normalised in `parse`; read it through [`Config::revive_policy`]). The player
    /// build always uses `"redown"`.
    pub rival_revive: String,
    /// T-039: seconds a rival hit takes off a downed LOCAL player's bleed-out clock (clamped to 0.5..=15.0 in `parse`).
    pub rival_hit_seconds: f32,
    /// RW-4: real seconds of the free Day 1 (0 = off; 120..=3600 otherwise, clamped in `parse`).
    pub day1_secs: u32,
    /// RW-1: seconds from a defeat to the defeat screen (0 = never; clamped to 0..=600 in `parse`).
    pub defeat_exit_secs: u32,
    /// GRACE-1: seconds after a PvP fight during which Sites of Grace are switched off (0 = rule off; clamped to 0..=120 in `parse`).
    pub grace_combat_secs: u32,
    /// RW-1: a defeated player's camera follows a living player.
    pub spectate: bool,
    /// RW-6: the private match code ("" = the public queue; normalised in `parse`).
    pub private_code: String,
    /// F3: move the LOCAL player to its team's own point at the hawk drop.
    pub team_spawns: bool,
    /// F3: metres from the drop point (clamped to `spawn::RADIUS_RANGE` in `parse`).
    pub spawn_radius: f32,
    /// UW-1: no 16:9 letterbox (`game::ultrawide`).
    pub ultrawide: bool,
    /// T-018: absolute log directory; empty = next to the DLL.
    pub log_dir: String,
    /// HAWK-2: the hawk crossing (`[hawk]`).
    pub hawk: HawkConfig,
}

/// HAWK-2: `[hawk]` in `nrpvp.toml`, the hawk crossing (PROJECT_PLAN §13 v1). Defaults = the values accepted in HAWK-4 (2026-10-22).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct HawkConfig {
    /// Start rides cruise the route, dive on a held stick (false = the vanilla start ride).
    pub enabled: bool,
    /// Real `GetBirdMovementDirectionTime` (394) value that starts the dive (clamped to [`HAWK_DIVE_HOLD_RANGE`] in `parse`).
    pub dive_hold: u32,
    /// Dive length in seconds (clamped to [`HAWK_DIVE_SECS_RANGE`] in `parse`).
    pub dive_secs: f32,
    /// The hawk's x1.3 speed (SpEffect 99230 check) during a dive.
    pub dive_boost: bool,
    /// Skip the tree ride's start animation.
    pub start_cut: bool,
}

/// The keys of [`HawkConfig`] (anything else in `[hawk]` is reported by `parse`).
const HAWK_KEYS: [&str; 5] = [
    "enabled",
    "dive_hold",
    "dive_secs",
    "dive_boost",
    "start_cut",
];
const HAWK_DIVE_HOLD_RANGE: (u64, u64) = (50, 5000);
/// Upper bound: a dive at x1.3 covers about 23 m/s, and the line stays about 150 m inside the land (PROJECT_PLAN §13).
const HAWK_DIVE_SECS_RANGE: (f32, f32) = (1.0, 15.0);
const HAWK_DIVE_SECS_DEFAULT: f32 = 7.0;

impl Default for HawkConfig {
    fn default() -> Self {
        HawkConfig {
            enabled: true,
            dive_hold: 400,
            dive_secs: HAWK_DIVE_SECS_DEFAULT,
            dive_boost: true,
            start_cut: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            dev_mode: false,
            team_candidates: DEFAULT_CANDIDATES.to_vec(),
            persist_interval_ms: DEFAULT_PERSIST_MS,
            watch_interval_ms: DEFAULT_WATCH_MS,
            pvpve: false,
            pvpve_team: DEFAULT_PVPVE_TEAM,
            pvpve_interval_ms: DEFAULT_PVPVE_INTERVAL_MS,
            pvpve_scheme: SCHEME_RIVALS.to_string(),
            team_size: DEFAULT_TEAM_SIZE,
            bots_fill: 0,
            auto_team_all: 0,
            mm_probe: false,
            net: true,
            allow_multi_instance: false,
            mm_queue: false,
            mm_match_players: DEFAULT_MM_MATCH_PLAYERS,
            mm_max_players: DEFAULT_MM_MAX_PLAYERS,
            mm_min_players: DEFAULT_MM_MIN_PLAYERS,
            mm_fill_wait_s: DEFAULT_MM_FILL_WAIT_S,
            matchmaking: true,
            mm_join: false,
            mm_host_create: false,
            mm_quiet_invite: false,
            mm_auto_start: false,
            mm_join_auto: "invite".to_string(),
            mm_lobby_trace: false,
            ui_text_trace: false,
            ui_flag_watch: false,
            toast_trace: false,
            ui_menu_trace: false,
            br_menu: false,
            mode: "br".to_string(),
            ui_msg_test: false,
            ui_messages: true,
            mm_sc_create: false,
            mm_sc_create_type: "public".to_string(),
            pos_candidates: crate::game::poswrite::default_candidates(),
            rival_revive: REVIVE_REDOWN.to_string(),
            rival_hit_seconds: crate::game::combat_state::DEFAULT_RIVAL_HIT_SECONDS,
            day1_secs: DEFAULT_DAY1_SECS,
            defeat_exit_secs: DEFAULT_DEFEAT_EXIT_SECS,
            grace_combat_secs: DEFAULT_GRACE_COMBAT_SECS,
            spectate: true,
            private_code: String::new(),
            team_spawns: false,
            spawn_radius: crate::game::spawn::DEFAULT_RADIUS,
            ultrawide: true,
            log_dir: String::new(),
            hawk: HawkConfig::default(),
        }
    }
}

/// RW-4: the free Day 1 lasts this long by default (the vanilla day is about 14 minutes, then the night).
pub(crate) const DEFAULT_DAY1_SECS: u32 = 1020;
pub(crate) const DAY1_SECS_RANGE: (u32, u32) = (120, 3600);
/// RW-1: default seconds between a defeat and the defeat screen.
pub(crate) const DEFAULT_DEFEAT_EXIT_SECS: u32 = 0;
pub(crate) const DEFEAT_EXIT_MAX: u32 = 600;
pub(crate) const DEFAULT_GRACE_COMBAT_SECS: u32 = 15;
pub(crate) const GRACE_COMBAT_MAX: u32 = 120;
/// RW-6: longest private match code.
pub(crate) const PRIVATE_CODE_MAX: usize = 32;

/// RW-6: a private match code as it is compared: trimmed and lower case; letters, digits, `-` and `_` only, 32 at most. Empty is
/// allowed (the public queue).
pub(crate) fn normalize_private_code(raw: &str) -> Result<String, String> {
    let t = raw.trim().to_ascii_lowercase();
    if t.len() > PRIVATE_CODE_MAX {
        return Err(format!("is longer than {PRIVATE_CODE_MAX} characters"));
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("may only hold letters, digits, - and _".to_string());
    }
    Ok(t)
}

/// T-026: which players the PvPvE mode rewrites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scheme {
    /// Only rivals of the local player (default).
    Rivals,
    /// Every player with value 2 (T-020 behaviour).
    All,
}

impl Scheme {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Scheme::Rivals => SCHEME_RIVALS,
            Scheme::All => SCHEME_ALL,
        }
    }
}

const SCHEME_RIVALS: &str = "rivals";
/// T-035: `rival_revive` values.
const REVIVE_REDOWN: &str = "redown";
const REVIVE_VANILLA: &str = "vanilla";
const SCHEME_ALL: &str = "all";
const DEFAULT_TEAM_SIZE: u64 = 1;
const TEAM_SIZE_RANGE: (u64, u64) = (1, 3);
/// BOT-1: at most 5 bots (a 6-player Solo match with one human).
const BOTS_FILL_RANGE: (u64, u64) = (0, 5);

impl Config {
    /// T-035: the rival revive policy. Always `Redown` outside dev mode.
    pub(crate) fn revive_policy(&self) -> crate::game::combat_state::RevivePolicy {
        if self.dev_mode && self.rival_revive == REVIVE_VANILLA {
            crate::game::combat_state::RevivePolicy::Vanilla
        } else {
            crate::game::combat_state::RevivePolicy::Redown
        }
    }

    /// OFF-1: the game mode the mod starts in (an unknown text, only possible on an unvalidated `Config`, means Battle Royale).
    pub(crate) fn game_mode(&self) -> crate::features::mode::GameMode {
        crate::features::mode::GameMode::parse(&self.mode).unwrap_or_default()
    }

    /// T-026: the scheme (an unknown text, only possible on an unvalidated `Config`, means rivals).
    pub(crate) fn scheme(&self) -> Scheme {
        if self.pvpve_scheme == SCHEME_ALL {
            Scheme::All
        } else {
            Scheme::Rivals
        }
    }

    /// MM-3: the automatic join action (an unknown text, only possible on an unvalidated `Config`, means `invite`).
    pub(crate) fn join_auto(&self) -> crate::mm::join::JoinAuto {
        crate::mm::join::JoinAuto::parse(&self.mm_join_auto)
            .unwrap_or(crate::mm::join::JoinAuto::Invite)
    }

    /// MM-3d: the lobby type of the create test (an unknown text, only possible on an unvalidated `Config`, means public).
    pub(crate) fn sc_create_type(&self) -> crate::steam::queue::ScType {
        crate::steam::queue::ScType::parse_create(&self.mm_sc_create_type)
            .unwrap_or(crate::steam::queue::ScType::Public)
    }

    /// T-026: players per team, within 1..=3.
    pub(crate) fn team_size_usize(&self) -> usize {
        usize::try_from(self.team_size.clamp(TEAM_SIZE_RANGE.0, TEAM_SIZE_RANGE.1)).unwrap_or(1)
    }
}

/// T-030: the player build fixes the mode: PvPvE on, rivals scheme, no probes. `explicit_off` / `explicit_all` say
/// whether the file itself asked for `pvpve = false` / `pvpve_scheme = "all"` (only then is a log line worth writing).
fn enforce_player_mode(
    cfg: &mut Config,
    explicit_off: bool,
    explicit_all: bool,
    warnings: &mut Vec<String>,
) {
    if cfg.dev_mode {
        return;
    }
    if explicit_off {
        warnings.push("pvpve = false is ignored: PvP is always on in the player build".to_string());
    }
    if explicit_all {
        warnings.push(
            "pvpve_scheme = \"all\" is ignored: the player build always uses \"rivals\""
                .to_string(),
        );
    }
    cfg.pvpve = true;
    cfg.pvpve_scheme = SCHEME_RIVALS.to_string();
    // MM-4: the player setting `matchmaking` (default true) turns on exactly what Solo matchmaking needs: the queue, the join step
    // with the joiner's link, and the quiet invite (S9 5g). `matchmaking = false` leaves all of it off.
    cfg.mm_queue = cfg.matchmaking;
    cfg.mm_join = cfg.matchmaking;
    cfg.mm_quiet_invite = cfg.matchmaking;
    cfg.mm_auto_start = cfg.matchmaking;
    // No Steam invite on the joiner's screen: the link alone, and the invite only if the link did not bring the player in.
    cfg.mm_join_auto = "link+invite".to_string();
    // MM-1: the Steam probe's logging is a developer tool (the callbacks it registers stay when the queue needs them).
    cfg.mm_probe = false;
    // NET-2: features (RW-5) need the data channel in a match; it only talks to the members of your Seamless Co-op lobby.
    cfg.net = cfg.matchmaking;
    cfg.allow_multi_instance = false;
    // MM-3e: the host-side lobby creation failed in FS-MM2 and is a developer tool.
    cfg.mm_host_create = false;
    // MM-3c: and the lobby tracer.
    cfg.mm_lobby_trace = false;
    // UI-0: and the text-id tracer.
    cfg.ui_text_trace = false;
    cfg.ui_flag_watch = false;
    cfg.toast_trace = false;
    cfg.ui_menu_trace = false;
    // UI-1 option 1: Battle Royale from the stock expedition menu comes with matchmaking (it needs the queue; the debug registers are
    // shared with the quiet invite through crate::hwbp: slots 0/1 quiet invite, 2 Commence, 3 labels). UI-2 `ui_messages` stays as written.
    cfg.br_menu = cfg.matchmaking;
    cfg.ui_msg_test = false;
    // MM-3d: and the SC-adoption test button.
    cfg.mm_sc_create = false;
    // T-035: the rival revive rule is always on.
    if cfg.rival_revive == REVIVE_VANILLA {
        warnings.push(
            "rival_revive = \"vanilla\" is ignored: the player build always uses \"redown\""
                .to_string(),
        );
    }
    cfg.rival_revive = REVIVE_REDOWN.to_string();
}

/// What the file set explicitly (before defaults and the alias): `pvpve = false`, `pvpve_scheme = "all"`.
fn peek_pvpve_overrides(text: &str) -> (bool, bool) {
    #[derive(Deserialize)]
    struct Peek {
        pvpve: Option<bool>,
        pvpve_scheme: Option<String>,
    }
    match toml::from_str::<Peek>(text) {
        Ok(p) => (
            p.pvpve == Some(false),
            p.pvpve_scheme
                .is_some_and(|s| s.trim().eq_ignore_ascii_case(SCHEME_ALL)),
        ),
        Err(_) => (false, false),
    }
}

/// Parse and sanitise. `Err` only for TOML/type errors; out-of-range values and unknown key names are
/// fixed and reported in the returned warnings.
pub(crate) fn parse(text: &str) -> Result<(Config, Vec<String>), String> {
    let mut cfg: Config = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut warnings = Vec::new();
    // R1: a player DLL (built without the `dev` feature) has no developer tooling, so it never runs in developer mode, whatever the
    // file says.
    if !cfg!(feature = "dev") && cfg.dev_mode {
        warnings.push(
            "dev_mode = true is ignored: this is a player build (no developer tooling)".to_string(),
        );
        cfg.dev_mode = false;
    }
    if cfg.team_candidates.is_empty() {
        warnings.push("team_candidates is empty; using the defaults".to_string());
        cfg.team_candidates = DEFAULT_CANDIDATES.to_vec();
    }
    let mut clamp = |name: &str, v: &mut u64, (lo, hi): (u64, u64)| {
        let c = (*v).clamp(lo, hi);
        if c != *v {
            warnings.push(format!("{name} = {v} is outside {lo}..={hi}; using {c}"));
            *v = c;
        }
    };
    clamp(
        "persist_interval_ms",
        &mut cfg.persist_interval_ms,
        PERSIST_MS_RANGE,
    );
    clamp(
        "watch_interval_ms",
        &mut cfg.watch_interval_ms,
        WATCH_MS_RANGE,
    );
    clamp(
        "pvpve_interval_ms",
        &mut cfg.pvpve_interval_ms,
        PVPVE_INTERVAL_MS_RANGE,
    );
    clamp("team_size", &mut cfg.team_size, TEAM_SIZE_RANGE);
    clamp("bots_fill", &mut cfg.bots_fill, BOTS_FILL_RANGE);
    clamp(
        "mm_match_players",
        &mut cfg.mm_match_players,
        MM_MATCH_PLAYERS_RANGE,
    );
    // MM-4: lobby size, smallest match, fill wait (the older `mm_match_players`, when the file sets it, is applied below).
    clamp(
        "mm_max_players",
        &mut cfg.mm_max_players,
        MM_MATCH_PLAYERS_RANGE,
    );
    clamp(
        "mm_min_players",
        &mut cfg.mm_min_players,
        MM_MATCH_PLAYERS_RANGE,
    );
    clamp(
        "mm_fill_wait_s",
        &mut cfg.mm_fill_wait_s,
        MM_FILL_WAIT_RANGE,
    );
    // MM-4: `mm_match_players`, when the file sets it, fixes min = max (the mmtest zip keeps 2 so tests stay quick).
    if let Ok(t) = toml::from_str::<toml::Table>(text) {
        if t.contains_key("mm_match_players") {
            if t.contains_key("mm_max_players") || t.contains_key("mm_min_players") {
                warnings.push("mm_match_players is set, so mm_min_players and mm_max_players are ignored (both become mm_match_players)".to_string());
            }
            cfg.mm_max_players = cfg.mm_match_players;
            cfg.mm_min_players = cfg.mm_match_players;
        }
    }
    if cfg.mm_min_players > cfg.mm_max_players {
        warnings.push(format!(
            "mm_min_players = {} is above mm_max_players = {}; using {}",
            cfg.mm_min_players, cfg.mm_max_players, cfg.mm_max_players
        ));
        cfg.mm_min_players = cfg.mm_max_players;
    }
    // T-032: position candidates; invalid entries are skipped with a warning, an empty result uses the defaults.
    {
        let (valid, mut w) = crate::game::poswrite::parse_list(&cfg.pos_candidates);
        warnings.append(&mut w);
        if valid.is_empty() {
            warnings.push("pos_candidates has no valid entry; using the defaults".to_string());
            cfg.pos_candidates = crate::game::poswrite::default_candidates();
        } else {
            cfg.pos_candidates = valid.iter().map(|c| c.text()).collect();
        }
    }
    // OFF-1: the game mode.
    match crate::features::mode::GameMode::parse(&cfg.mode) {
        Some(m) => cfg.mode = m.label().to_string(),
        None => {
            warnings.push(format!(
                "mode = {:?} is not \"br\" or \"offline\"; using \"br\"",
                cfg.mode
            ));
            cfg.mode = "br".to_string();
        }
    }
    // T-035: the rival revive policy.
    {
        let f = cfg.rival_revive.trim().to_ascii_lowercase();
        if f == REVIVE_REDOWN || f == REVIVE_VANILLA {
            cfg.rival_revive = f;
        } else {
            warnings.push(format!(
                "rival_revive = {:?} is not \"redown\" or \"vanilla\"; using \"{REVIVE_REDOWN}\"",
                cfg.rival_revive
            ));
            cfg.rival_revive = REVIVE_REDOWN.to_string();
        }
    }
    // MM-3: mm_join_auto.
    {
        match crate::mm::join::JoinAuto::parse(&cfg.mm_join_auto) {
            Some(a) => cfg.mm_join_auto = a.label().to_string(),
            None => {
                warnings.push(format!(
                    "mm_join_auto = {:?} is not \"invite\", \"invite+link\", \"link\", \"link+invite\" or \"none\"; using \"invite\"",
                    cfg.mm_join_auto
                ));
                cfg.mm_join_auto = "invite".to_string();
            }
        }
        if cfg.dev_mode && cfg.mm_join && !cfg.mm_queue {
            warnings.push("mm_join = true needs mm_queue = true; the join step is off".to_string());
            cfg.mm_join = false;
        }
        if cfg.dev_mode && cfg.mm_host_create && !cfg.mm_join {
            warnings.push(
                "mm_host_create = true needs mm_join = true; the host-side lobby creation is off"
                    .to_string(),
            );
            cfg.mm_host_create = false;
        }
        if cfg.dev_mode && cfg.mm_quiet_invite && !cfg.mm_join {
            warnings.push(
                "mm_quiet_invite = true needs mm_join = true; the invite-window suppression is off"
                    .to_string(),
            );
            cfg.mm_quiet_invite = false;
        }
        // MM-3f: both use the four debug registers of every thread; the tracer would overwrite the suppression's.
        if cfg.dev_mode && cfg.mm_auto_start && !cfg.mm_join {
            warnings.push(
                "mm_auto_start = true needs mm_join = true; the automatic start is off".to_string(),
            );
            cfg.mm_auto_start = false;
        }
        if cfg.dev_mode && cfg.mm_quiet_invite && cfg.mm_lobby_trace {
            warnings.push("mm_quiet_invite and mm_lobby_trace both use the hardware breakpoints; mm_quiet_invite is off while mm_lobby_trace = true".to_string());
            cfg.mm_quiet_invite = false;
        }
        // UI-1 option 1: br_menu shares the debug registers through crate::hwbp (with mm_quiet_invite); the dev tools that take all
        // four registers win, and br_menu needs the queue worker.
        if cfg.dev_mode
            && cfg.br_menu
            && (cfg.ui_menu_trace || cfg.ui_text_trace || cfg.mm_lobby_trace)
        {
            warnings.push("br_menu is off while ui_menu_trace, ui_text_trace or mm_lobby_trace is on (they own all debug registers)".to_string());
            cfg.br_menu = false;
        }
        if cfg.dev_mode && cfg.br_menu && !cfg.mm_queue {
            warnings
                .push("br_menu = true needs mm_queue = true; Commence is left alone".to_string());
            cfg.br_menu = false;
        }
        // UI-1: the menu tracer owns all four debug registers.
        if cfg.dev_mode
            && cfg.ui_menu_trace
            && (cfg.ui_text_trace || cfg.mm_quiet_invite || cfg.mm_lobby_trace)
        {
            warnings.push("ui_menu_trace uses all hardware breakpoints; ui_text_trace, mm_quiet_invite and mm_lobby_trace are off while it is on".to_string());
            cfg.ui_text_trace = false;
            cfg.mm_quiet_invite = false;
            cfg.mm_lobby_trace = false;
        }
        // UI-0: the text tracer owns the debug registers too.
        if cfg.dev_mode && cfg.ui_text_trace && (cfg.mm_quiet_invite || cfg.mm_lobby_trace) {
            warnings.push("ui_text_trace uses the hardware breakpoints; mm_quiet_invite and mm_lobby_trace are off while it is on".to_string());
            cfg.mm_quiet_invite = false;
            cfg.mm_lobby_trace = false;
        }
    }
    // MM-3d: mm_sc_create_type, and the button needs the Steam worker (mm_queue).
    {
        match crate::steam::queue::ScType::parse_create(&cfg.mm_sc_create_type) {
            Some(t) => cfg.mm_sc_create_type = t.create_name().to_string(),
            None => {
                warnings.push(format!(
                    "mm_sc_create_type = {:?} is not \"public\", \"friends\" or \"private\"; using \"public\"",
                    cfg.mm_sc_create_type
                ));
                cfg.mm_sc_create_type = "public".to_string();
            }
        }
        if cfg.dev_mode && cfg.mm_sc_create && !cfg.mm_queue {
            warnings
                .push("mm_sc_create = true needs mm_queue = true; the button is off".to_string());
            cfg.mm_sc_create = false;
        }
    }
    // T-039: rival_hit_seconds.
    {
        let (lo, hi) = crate::game::combat_state::RIVAL_HIT_SECONDS_RANGE;
        let v = cfg.rival_hit_seconds;
        if !v.is_finite() {
            warnings
                .push("rival_hit_seconds is not a finite number; using the default".to_string());
            cfg.rival_hit_seconds = crate::game::combat_state::DEFAULT_RIVAL_HIT_SECONDS;
        } else if !(lo..=hi).contains(&v) {
            cfg.rival_hit_seconds = v.clamp(lo, hi);
            warnings.push(format!(
                "rival_hit_seconds = {v} is outside {lo}..={hi}; using {}",
                cfg.rival_hit_seconds
            ));
        }
    }
    // RW-4: day1_secs (0 = off).
    if cfg.day1_secs != 0 && !(DAY1_SECS_RANGE.0..=DAY1_SECS_RANGE.1).contains(&cfg.day1_secs) {
        let v = cfg.day1_secs;
        cfg.day1_secs = v.clamp(DAY1_SECS_RANGE.0, DAY1_SECS_RANGE.1);
        warnings.push(format!(
            "day1_secs = {v} is outside {}..={} (or 0); using {}",
            DAY1_SECS_RANGE.0, DAY1_SECS_RANGE.1, cfg.day1_secs
        ));
    }
    // RW-1: defeat_exit_secs.
    if cfg.defeat_exit_secs > DEFEAT_EXIT_MAX {
        warnings.push(format!(
            "defeat_exit_secs = {} is above {DEFEAT_EXIT_MAX}; using {DEFEAT_EXIT_MAX}",
            cfg.defeat_exit_secs
        ));
        cfg.defeat_exit_secs = DEFEAT_EXIT_MAX;
    }
    // GRACE-1: grace_combat_secs (0 = off).
    if cfg.grace_combat_secs > GRACE_COMBAT_MAX {
        warnings.push(format!(
            "grace_combat_secs = {} is above {GRACE_COMBAT_MAX}; using {GRACE_COMBAT_MAX}",
            cfg.grace_combat_secs
        ));
        cfg.grace_combat_secs = GRACE_COMBAT_MAX;
    }
    // RW-6: private_code.
    match normalize_private_code(&cfg.private_code) {
        Ok(code) => cfg.private_code = code,
        Err(e) => {
            warnings.push(format!("private_code {e}; using the public queue"));
            cfg.private_code = String::new();
        }
    }
    // F3: spawn_radius.
    {
        let (lo, hi) = crate::game::spawn::RADIUS_RANGE;
        let v = cfg.spawn_radius;
        if !v.is_finite() {
            warnings.push("spawn_radius is not a finite number; using the default".to_string());
            cfg.spawn_radius = crate::game::spawn::DEFAULT_RADIUS;
        } else if !(lo..=hi).contains(&v) {
            cfg.spawn_radius = v.clamp(lo, hi);
            warnings.push(format!(
                "spawn_radius = {v} is outside {lo}..={hi}; using {}",
                cfg.spawn_radius
            ));
        }
    }
    // HAWK-2: [hawk] dive_hold and dive_secs; keys that only landed in [hawk] because they were added below it.
    {
        if let Ok(t) = toml::from_str::<toml::Table>(text) {
            if let Some(toml::Value::Table(h)) = t.get("hawk") {
                let stray: Vec<&str> = h
                    .keys()
                    .map(String::as_str)
                    .filter(|k| !HAWK_KEYS.contains(k))
                    .collect();
                if !stray.is_empty() {
                    warnings.push(format!(
                        "[hawk] has no setting {}; ignored (settings outside [hawk] must come before the [hawk] line)",
                        stray.join(" / ")
                    ));
                }
            }
        }
        let mut hold = u64::from(cfg.hawk.dive_hold);
        let (lo, hi) = HAWK_DIVE_HOLD_RANGE;
        if !(lo..=hi).contains(&hold) {
            let c = hold.clamp(lo, hi);
            warnings.push(format!(
                "[hawk] dive_hold = {hold} is outside {lo}..={hi}; using {c}"
            ));
            hold = c;
        }
        cfg.hawk.dive_hold = u32::try_from(hold).unwrap_or(400);
        let (lo, hi) = HAWK_DIVE_SECS_RANGE;
        let v = cfg.hawk.dive_secs;
        if !v.is_finite() {
            warnings.push("[hawk] dive_secs is not a finite number; using the default".to_string());
            cfg.hawk.dive_secs = HAWK_DIVE_SECS_DEFAULT;
        } else if !(lo..=hi).contains(&v) {
            cfg.hawk.dive_secs = v.clamp(lo, hi);
            warnings.push(format!(
                "[hawk] dive_secs = {v} is outside {lo}..={hi}; using {}",
                cfg.hawk.dive_secs
            ));
        }
    }
    // T-035: the T-033/T-034 finisher experiment is gone; its keys are ignored (one line).
    if let Ok(t) = toml::from_str::<toml::Table>(text) {
        let old: Vec<&str> = ["finisher", "hotkey_finisher"]
            .into_iter()
            .filter(|k| t.contains_key(*k))
            .collect();
        if !old.is_empty() {
            warnings.push(format!(
                "{} no longer exist(s) (the finisher experiment was replaced by the always-on rival revive rule, T-035); ignored",
                old.join(" / ")
            ));
        }
    }
    let scheme = cfg.pvpve_scheme.trim().to_ascii_lowercase();
    if scheme == SCHEME_RIVALS || scheme == SCHEME_ALL {
        cfg.pvpve_scheme = scheme;
    } else {
        warnings.push(format!(
            "pvpve_scheme = {:?} is not \"rivals\" or \"all\"; using \"{SCHEME_RIVALS}\"",
            cfg.pvpve_scheme
        ));
        cfg.pvpve_scheme = SCHEME_RIVALS.to_string();
    }
    // The mode only replaces the expedition default (2), so 2 would be a no-op and 0 is "None".
    if cfg.pvpve_team == 0 || cfg.pvpve_team == 2 {
        warnings.push(format!(
            "pvpve_team = {} is not usable (0 = None, 2 = the value it replaces); using {DEFAULT_PVPVE_TEAM}",
            cfg.pvpve_team
        ));
        cfg.pvpve_team = DEFAULT_PVPVE_TEAM;
    }
    // Deprecated alias: auto_team_all = N means pvpve = true, pvpve_team = N, unless pvpve is set explicitly.
    if cfg.auto_team_all != 0 {
        let pvpve_set = pvpve_is_set(text);
        match u8::try_from(cfg.auto_team_all) {
            _ if pvpve_set => warnings.push(format!(
                "auto_team_all = {} is deprecated and ignored because pvpve is set",
                cfg.auto_team_all
            )),
            Ok(n) if n != 2 => {
                warnings.push(format!(
                    "auto_team_all = {n} is deprecated; treating it as pvpve = true, pvpve_team = {n} (set pvpve explicitly)"
                ));
                cfg.pvpve = true;
                cfg.pvpve_team = n;
            }
            _ => warnings.push(format!(
                "auto_team_all = {} is deprecated and not a usable team value (1..=255 except 2); ignored",
                cfg.auto_team_all
            )),
        }
    }

    if !cfg.log_dir.trim().is_empty() && !Path::new(cfg.log_dir.trim()).is_absolute() {
        warnings.push(format!(
            "log_dir = {:?} is not an absolute path; logging next to the DLL",
            cfg.log_dir
        ));
    }

    // R5: settings removed with their features are ignored, named once so an old file is easy to tidy.
    if let Ok(t) = toml::from_str::<toml::Table>(text) {
        let gone: Vec<&str> = t
            .keys()
            .map(String::as_str)
            .filter(|k| {
                (k.starts_with("hotkey_") && *k != "hotkey_finisher")
                    || k.starts_with("overlay_")
                    || matches!(
                        *k,
                        "probes"
                            | "probe_interval_ms"
                            | "ui_c1_proto"
                            | "downed_rival_team"
                            | "downed_rival_candidates"
                    )
            })
            .collect();
        if !gone.is_empty() {
            warnings.push(format!(
                "ignored (removed with the overlay, the hotkeys and the probes): {}",
                gone.join(", ")
            ));
        }
    }
    let (explicit_off, explicit_all) = peek_pvpve_overrides(text);
    enforce_player_mode(&mut cfg, explicit_off, explicit_all, &mut warnings);

    Ok((cfg, warnings))
}

/// True if the file sets `pvpve` explicitly (so the deprecated `auto_team_all` alias must not override it).
fn pvpve_is_set(text: &str) -> bool {
    #[derive(Deserialize)]
    struct Peek {
        pvpve: Option<bool>,
    }
    toml::from_str::<Peek>(text).is_ok_and(|p| p.pvpve.is_some())
}

/// T-018: the raw `log_dir` from `<dir>/nrpvp.toml`, read before logging exists. Any problem (no file,
/// bad TOML, wrong type) gives `None` (= log next to the DLL); `load` reports parse errors later.
pub(crate) fn peek_log_dir(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(FILE_NAME)).ok()?;
    parse_log_dir(&text)
}

fn parse_log_dir(text: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Peek {
        log_dir: Option<String>,
    }
    let p: Peek = toml::from_str(text).ok()?;
    p.log_dir.filter(|s| !s.trim().is_empty())
}

/// The config used when the file is missing or unreadable: the defaults, with the player-mode rules applied.
fn fallback() -> Config {
    let mut cfg = Config::default();
    enforce_player_mode(&mut cfg, false, false, &mut Vec::new());
    cfg
}

/// Load `<dir>/nrpvp.toml`, creating it with defaults if it does not exist. Never fails.
pub(crate) fn load(dir: &Path) -> Config {
    let path = dir.join(FILE_NAME);
    let cfg = match std::fs::read_to_string(&path) {
        Ok(text) => match parse(&text) {
            Ok((cfg, warnings)) => {
                tracing::info!("T-008: read config {}", path.display());
                for w in warnings {
                    tracing::warn!("T-008: config: {w}");
                }
                cfg
            }
            Err(e) => {
                tracing::warn!(
                    "T-008: could not parse {}: {e}; using the defaults",
                    path.display()
                );
                fallback()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::write(&path, DEFAULT_FILE_TEXT) {
                Ok(()) => tracing::info!("T-008: created default config {}", path.display()),
                Err(e) => tracing::warn!(
                    "T-008: could not create {}: {e}; using the defaults",
                    path.display()
                ),
            }
            fallback()
        }
        Err(e) => {
            tracing::warn!(
                "T-008: could not read {}: {e}; using the defaults",
                path.display()
            );
            fallback()
        }
    };
    tracing::info!(
        "build mode: {}",
        if cfg.dev_mode { "DEV" } else { "PLAYER" }
    );
    tracing::info!(
        "T-008: effective config: team_candidates={:?} persist_interval_ms={} watch_interval_ms={} pvpve={} pvpve_team={} pvpve_interval_ms={} pvpve_scheme={} team_size={} matchmaking={} mm_queue={} mm_match_players={} mm_max_players={} mm_min_players={} mm_fill_wait_s={} mm_join={} mm_join_auto={} mm_quiet_invite={} br_menu={} mode={} log_dir={:?}",
        cfg.team_candidates,
        cfg.persist_interval_ms,
        cfg.watch_interval_ms,
        cfg.pvpve,
        cfg.pvpve_team,
        cfg.pvpve_interval_ms,
        cfg.pvpve_scheme,
        cfg.team_size,
        cfg.matchmaking,
        cfg.mm_queue,
        cfg.mm_match_players,
        cfg.mm_max_players,
        cfg.mm_min_players,
        cfg.mm_fill_wait_s,
        cfg.mm_join,
        cfg.mm_join_auto,
        cfg.mm_quiet_invite,
        cfg.br_menu,
        cfg.mode,
        cfg.log_dir
    );
    tracing::info!("T-008: effective config: [hawk] {:?}", cfg.hawk);
    cfg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::combat_state::RevivePolicy;

    /// What a player-mode parse of an empty file gives.
    fn player_default() -> Config {
        // MM-4: matchmaking = true (the default) turns on the queue, the join step with the link first, the quiet invite and the automatic start; UI-1: and
        // Battle Royale from the expedition menu.
        Config {
            pvpve: true,
            mm_queue: true,
            mm_join: true,
            mm_join_auto: "link+invite".to_string(),
            mm_auto_start: true,
            mm_quiet_invite: true,
            br_menu: true,
            ..Config::default()
        }
    }

    fn dev_default() -> Config {
        Config {
            dev_mode: true,
            ..Config::default()
        }
    }

    fn parse_dev(text: &str) -> Result<(Config, Vec<String>), String> {
        parse(&format!(
            "dev_mode = true
{text}"
        ))
    }

    #[test]
    fn default_file_text_parses_to_defaults() {
        let (cfg, warnings) = parse(DEFAULT_FILE_TEXT).expect("default text parses");
        assert_eq!(cfg, player_default());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(cfg.team_candidates, vec![6, 29, 3, 16, 13, 2, 1]);
        assert_eq!((cfg.persist_interval_ms, cfg.watch_interval_ms), (50, 1000));
    }

    #[test]
    fn log_dir_parses_and_peeks() {
        assert_eq!(dev_default().log_dir, "");
        let (cfg, w) = parse_dev("log_dir = 'D:\\repo\\dist\\nucleus-logs'\n").expect("ok");
        assert_eq!(cfg.log_dir, "D:\\repo\\dist\\nucleus-logs");
        assert!(w.is_empty(), "{w:?}");
        // The peek ignores type errors in other keys (the full parse reports them later).
        assert_eq!(
            parse_log_dir("overlay_enabled = \"junk\"\nlog_dir = 'D:\\x'\n"),
            Some("D:\\x".to_string())
        );
        assert_eq!(
            parse_log_dir("team_candidates = [1]\nlog_dir = 'D:\\x'\n"),
            Some("D:\\x".to_string())
        );
        assert_eq!(parse_log_dir("log_dir = \"\"\n"), None);
        assert_eq!(parse_log_dir("log_dir = 5"), None);
        assert_eq!(parse_log_dir("not toml"), None);
        assert_eq!(parse_log_dir(""), None);
        let (_, w) = parse_dev("log_dir = 'rel\\dir'\n").expect("ok");
        assert_eq!(w.len(), 1, "{w:?}");
    }

    #[test]
    fn pvpve_defaults_and_parsing() {
        let c = dev_default();
        assert_eq!(
            (c.pvpve, c.pvpve_team, c.pvpve_interval_ms),
            (false, 29, 250)
        );
        let (cfg, w) =
            parse_dev("pvpve = true\npvpve_team = 13\npvpve_interval_ms = 100\n").expect("ok");
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(
            (cfg.pvpve, cfg.pvpve_team, cfg.pvpve_interval_ms),
            (true, 13, 100)
        );
        // Out-of-range interval and unusable teams are fixed with warnings.
        let (cfg, w) = parse_dev("pvpve_interval_ms = 1\npvpve_team = 2\n").expect("ok");
        assert_eq!((cfg.pvpve_interval_ms, cfg.pvpve_team), (50, 29));
        assert_eq!(w.len(), 2, "{w:?}");
        let (cfg, w) = parse_dev("pvpve_team = 0\n").expect("ok");
        assert_eq!(cfg.pvpve_team, 29);
        assert_eq!(w.len(), 1);
        assert!(parse_dev("pvpve = \"yes\"").is_err());
        assert!(parse_dev("pvpve_team = 300").is_err());
    }

    #[test]
    fn mm_queue_defaults_off_is_dev_only() {
        let c = dev_default();
        assert!(!c.mm_queue && c.mm_match_players == 2);
        let (cfg, w) = parse_dev(
            "mm_queue = true
mm_match_players = 3
",
        )
        .expect("ok");
        assert!(
            cfg.mm_queue && cfg.mm_match_players == 3 && w.is_empty(),
            "{w:?}"
        );
        // Range validation.
        let (cfg, w) = parse_dev(
            "mm_queue = true
mm_match_players = 9
",
        )
        .expect("ok");
        assert_eq!(cfg.mm_match_players, 6);
        assert_eq!(w.len(), 1, "{w:?}");
        let (cfg, _) = parse_dev(
            "mm_match_players = 1
",
        )
        .expect("ok");
        assert_eq!(cfg.mm_match_players, 2);
        // Player build: the queue follows `matchmaking`, not `mm_queue`.
        let (cfg, _) = parse("mm_queue = true\nmatchmaking = false\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_queue);
        assert!(parse_dev("mm_queue = \"yes\"").is_err());
    }

    #[test]
    fn mm_join_defaults_off_is_dev_only_and_needs_the_queue() {
        use crate::mm::join::JoinAuto;
        let c = dev_default();
        assert!(!c.mm_join && c.join_auto() == JoinAuto::Invite);
        let (cfg, w) =
            parse_dev("mm_queue = true\nmm_join = true\nmm_join_auto = \"Invite+Link\"\n")
                .expect("ok");
        assert!(
            cfg.mm_join && cfg.join_auto() == JoinAuto::InviteLink && w.is_empty(),
            "{w:?}"
        );
        for (t, a) in [
            ("link", JoinAuto::Link),
            ("none", JoinAuto::None),
            ("invite", JoinAuto::Invite),
        ] {
            let (cfg, _) = parse_dev(&format!("mm_join_auto = \"{t}\"\n")).expect("ok");
            assert_eq!(cfg.join_auto(), a);
        }
        let (cfg, w) = parse_dev("mm_queue = true\nmm_join_auto = \"banana\"\n").expect("ok");
        assert_eq!((cfg.join_auto(), w.len()), (JoinAuto::Invite, 1), "{w:?}");
        // mm_join without mm_queue is switched off with a warning.
        let (cfg, w) = parse_dev("mm_join = true\n").expect("ok");
        assert!(!cfg.mm_join && w.len() == 1, "{w:?}");
        // Player build: follows `matchmaking`, and always joins with the link first.
        let (cfg, _) = parse("mm_queue = true\nmm_join = true\nmatchmaking = false\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_join);
        let (cfg, _) = parse("mm_join_auto = \"none\"\n").expect("ok");
        assert!(cfg.mm_join && cfg.join_auto() == JoinAuto::LinkThenInvite && cfg.mm_auto_start);
        assert!(parse_dev("mm_join = \"yes\"").is_err());
    }

    #[test]
    fn mm_host_create_defaults_off_is_dev_only_and_needs_mm_join() {
        assert!(!Config::default().mm_host_create);
        assert!(!dev_default().mm_host_create);
        let (cfg, w) =
            parse_dev("mm_queue = true\nmm_join = true\nmm_host_create = true\n").expect("ok");
        assert!(cfg.mm_host_create && w.is_empty(), "{w:?}");
        // Without mm_join the creation has no gate to run in.
        let (cfg, w) = parse_dev("mm_queue = true\nmm_host_create = true\n").expect("ok");
        assert!(!cfg.mm_host_create && w.len() == 1, "{w:?}");
        assert!(parse_dev("mm_host_create = \"yes\"").is_err());
        // The player build (no dev_mode) forces it off even when the file asks for it.
        let (cfg, _) =
            parse("mm_queue = true\nmm_join = true\nmm_host_create = true\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_host_create);
    }

    #[test]
    fn mm_quiet_invite_defaults_off_is_dev_only_needs_mm_join_and_excludes_the_tracer() {
        assert!(!Config::default().mm_quiet_invite);
        assert!(!dev_default().mm_quiet_invite);
        let (cfg, w) =
            parse_dev("mm_queue = true\nmm_join = true\nmm_quiet_invite = true\n").expect("ok");
        assert!(cfg.mm_quiet_invite && w.is_empty(), "{w:?}");
        let (cfg, w) = parse_dev("mm_queue = true\nmm_quiet_invite = true\n").expect("ok");
        assert!(!cfg.mm_quiet_invite && w.len() == 1, "{w:?}");
        let (cfg, w) = parse_dev(
            "mm_queue = true\nmm_join = true\nmm_quiet_invite = true\nmm_lobby_trace = true\n",
        )
        .expect("ok");
        assert!(
            !cfg.mm_quiet_invite && cfg.mm_lobby_trace && w.len() == 1,
            "{w:?}"
        );
        assert!(parse_dev("mm_quiet_invite = \"yes\"").is_err());
        let (cfg, _) =
            parse("mm_queue = true\nmm_join = true\nmm_quiet_invite = true\nmatchmaking = false\n")
                .expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_quiet_invite);
    }

    #[test]
    fn mm_probe_defaults_off_and_is_dev_only() {
        assert!(!Config::default().mm_probe);
        assert!(!dev_default().mm_probe);
        let (cfg, w) = parse_dev("mm_probe = true\n").expect("ok");
        assert!(cfg.mm_probe && w.is_empty(), "{w:?}");
        assert!(parse_dev("mm_probe = \"yes\"").is_err());
        // The player build (no dev_mode) forces it off even when the file asks for it.
        let (cfg, _) = parse("mm_probe = true\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_probe);
    }

    #[test]
    fn ui_text_trace_is_dev_only_and_takes_the_debug_registers() {
        assert!(!Config::default().ui_text_trace);
        let (cfg, w) = parse_dev(
            "ui_text_trace = true
",
        )
        .expect("ok");
        assert!(cfg.ui_text_trace && w.is_empty(), "{w:?}");
        let (cfg, w) =
            parse_dev("mm_queue = true\nmm_join = true\nmm_quiet_invite = true\nmm_lobby_trace = true\nui_text_trace = true\n")
                .expect("ok");
        assert!(
            cfg.ui_text_trace && !cfg.mm_quiet_invite && !cfg.mm_lobby_trace,
            "{w:?}"
        );
        let (cfg, _) = parse("ui_text_trace = true\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.ui_text_trace);
        let (cfg, w) = parse_dev("ui_flag_watch = true\n").expect("ok");
        assert!(cfg.ui_flag_watch && w.is_empty(), "{w:?}");
        let (cfg, _) = parse("ui_flag_watch = true\n").expect("ok");
        assert!(!cfg.ui_flag_watch);
        let (cfg, _) = parse_dev("toast_trace = true\n").expect("ok");
        assert!(cfg.toast_trace);
        let (cfg, _) = parse("toast_trace = true\n").expect("ok");
        assert!(!cfg.toast_trace);
        let (cfg, w) = parse_dev("ui_text_trace = true\nui_menu_trace = true\n").expect("ok");
        assert!(
            cfg.ui_menu_trace && !cfg.ui_text_trace && w.len() == 1,
            "{w:?}"
        );
        let (cfg, _) = parse("ui_menu_trace = true\n").expect("ok");
        assert!(!cfg.ui_menu_trace);
    }

    #[test]
    fn mode_defaults_to_br_and_accepts_offline_in_both_builds() {
        use crate::features::mode::GameMode;
        assert_eq!(Config::default().game_mode(), GameMode::BattleRoyale);
        let (cfg, w) = parse("mode = \"Offline\"\n").expect("ok");
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(
            (cfg.mode.as_str(), cfg.game_mode()),
            ("offline", GameMode::Offline)
        );
        // Offline keeps the Battle Royale menu on: it only changes what Commence does.
        assert!(cfg.br_menu);
        let (cfg, _) =
            parse_dev("mode = \"offline\"\nmm_queue = true\nbr_menu = true\n").expect("ok");
        assert!(cfg.br_menu && cfg.game_mode() == GameMode::Offline);
        let (cfg, w) = parse("mode = \"solo\"\n").expect("ok");
        assert_eq!(cfg.game_mode(), GameMode::BattleRoyale);
        assert!(w.iter().any(|l| l.contains("mode = \"solo\"")), "{w:?}");
    }

    #[test]
    fn br_menu_needs_the_queue_and_yields_to_the_dev_tracers() {
        assert!(!Config::default().br_menu);
        let (cfg, w) = parse_dev("mm_queue = true\nbr_menu = true\n").expect("ok");
        assert!(cfg.br_menu && w.is_empty(), "{w:?}");
        let (cfg, w) = parse_dev("br_menu = true\n").expect("ok");
        assert!(!cfg.br_menu && w.len() == 1, "{w:?}");
        let (cfg, _) =
            parse_dev("mm_queue = true\nbr_menu = true\nui_menu_trace = true\n").expect("ok");
        assert!(!cfg.br_menu);
        let (cfg, _) =
            parse_dev("mm_queue = true\nmm_join = true\nmm_quiet_invite = true\nbr_menu = true\n")
                .expect("ok");
        assert!(cfg.br_menu && cfg.mm_quiet_invite);
        // The player build: on with matchmaking (whatever the file says), next to the quiet invite; off without matchmaking.
        let (cfg, _) = parse("br_menu = false\n").expect("ok");
        assert!(cfg.br_menu && cfg.mm_queue && cfg.mm_quiet_invite && cfg.ui_messages);
        assert!(
            !cfg.ui_text_trace && !cfg.ui_menu_trace && !cfg.ui_msg_test && !cfg.mm_lobby_trace
        );
        let (cfg, _) = parse("ui_text_trace = true\nui_menu_trace = true\nui_msg_test = true\nmm_lobby_trace = true\n").expect("ok");
        assert!(cfg.br_menu && cfg.mm_quiet_invite);
        assert!(
            !cfg.ui_text_trace && !cfg.ui_menu_trace && !cfg.ui_msg_test && !cfg.mm_lobby_trace
        );
        let (cfg, _) = parse("matchmaking = false\nbr_menu = true\n").expect("ok");
        assert!(!cfg.br_menu && !cfg.mm_queue);
    }

    #[test]
    fn mm_lobby_trace_defaults_off_and_is_dev_only() {
        assert!(!Config::default().mm_lobby_trace);
        assert!(!dev_default().mm_lobby_trace);
        let (cfg, w) = parse_dev("mm_lobby_trace = true\n").expect("ok");
        assert!(cfg.mm_lobby_trace && w.is_empty(), "{w:?}");
        assert!(parse_dev("mm_lobby_trace = \"yes\"").is_err());
        // The player build (no dev_mode) forces it off even when the file asks for it.
        let (cfg, _) = parse("mm_lobby_trace = true\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_lobby_trace);
    }

    #[test]
    fn mm_sc_create_defaults_off_is_dev_only_and_needs_the_queue() {
        use crate::steam::queue::ScType;
        let c = Config::default();
        assert!(!c.mm_sc_create && c.sc_create_type() == ScType::Public);
        assert!(!dev_default().mm_sc_create);
        let (cfg, w) =
            parse_dev("mm_queue = true\nmm_sc_create = true\nmm_sc_create_type = \"Friends\"\n")
                .expect("ok");
        assert!(
            cfg.mm_sc_create && cfg.sc_create_type() == ScType::Friends && w.is_empty(),
            "{w:?}"
        );
        let (cfg, w) =
            parse_dev("mm_queue = true\nmm_sc_create_type = \"invisible\"\n").expect("ok");
        assert_eq!(
            (cfg.sc_create_type(), w.len()),
            (ScType::Public, 1),
            "{w:?}"
        );
        // Without the queue worker there is nothing to run it on.
        let (cfg, w) = parse_dev("mm_sc_create = true\n").expect("ok");
        assert!(!cfg.mm_sc_create && w.len() == 1, "{w:?}");
        assert!(parse_dev("mm_sc_create = \"yes\"").is_err());
        // The player build (no dev_mode) forces it off even when the file asks for it.
        let (cfg, _) = parse("mm_queue = true\nmm_sc_create = true\n").expect("ok");
        assert!(!cfg.dev_mode && !cfg.mm_sc_create);
    }

    #[test]
    fn scheme_and_team_size_parse_with_defaults_and_warnings() {
        let (c, w) = parse_dev("").expect("ok");
        assert_eq!((c.scheme(), c.team_size_usize()), (Scheme::Rivals, 1));
        assert!(w.is_empty());
        let (c, w) = parse_dev("pvpve_scheme = \"all\"\nteam_size = 3\n").expect("ok");
        assert_eq!((c.scheme(), c.team_size_usize()), (Scheme::All, 3));
        assert!(w.is_empty());
        let (c, _) = parse_dev("pvpve_scheme = \" ALL \"\n").expect("ok");
        assert_eq!(c.scheme(), Scheme::All);
        let (c, w) = parse_dev("pvpve_scheme = \"bogus\"\nteam_size = 9\n").expect("ok");
        assert_eq!((c.scheme(), c.team_size_usize()), (Scheme::Rivals, 3));
        assert_eq!(w.len(), 2, "{w:?}");
        let (c, w) = parse_dev("team_size = 0\n").expect("ok");
        assert_eq!(c.team_size_usize(), 1);
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn auto_team_all_is_a_deprecated_alias() {
        // Unset pvpve + non-zero alias: mode on with that team, one warning.
        let (cfg, w) = parse_dev("auto_team_all = 29\n").expect("ok");
        assert_eq!((cfg.pvpve, cfg.pvpve_team), (true, 29));
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("deprecated"));
        let (cfg, _) = parse_dev("auto_team_all = 6\n").expect("ok");
        assert_eq!((cfg.pvpve, cfg.pvpve_team), (true, 6));
        // Alias 0 does nothing and says nothing.
        let (cfg, w) = parse_dev("auto_team_all = 0\n").expect("ok");
        assert!(!cfg.pvpve && w.is_empty());
        // An explicit pvpve (even false) wins; the alias is ignored with a warning.
        let (cfg, w) = parse_dev("pvpve = false\nauto_team_all = 29\n").expect("ok");
        assert!(!cfg.pvpve);
        assert_eq!(w.len(), 1, "{w:?}");
        let (cfg, _) =
            parse_dev("pvpve = true\npvpve_team = 13\nauto_team_all = 29\n").expect("ok");
        assert_eq!((cfg.pvpve, cfg.pvpve_team), (true, 13));
        // A value that is not a usable team is ignored.
        let (cfg, w) = parse_dev("auto_team_all = 300\n").expect("ok");
        assert!(!cfg.pvpve);
        assert_eq!(w.len(), 1, "{w:?}");
        let (cfg, _) = parse_dev("auto_team_all = 2\n").expect("ok");
        assert!(!cfg.pvpve);
    }

    #[test]
    fn empty_file_gives_defaults() {
        let (cfg, w) = parse("").expect("empty parses");
        assert_eq!(cfg, player_default());
        assert!(w.is_empty());
    }

    #[test]
    fn partial_file_overrides_only_given_keys() {
        let (cfg, _) = parse("team_candidates = [13, 6]\nwatch_interval_ms = 250\n").expect("ok");
        assert_eq!(cfg.team_candidates, vec![13, 6]);
        assert_eq!(cfg.watch_interval_ms, 250);
        assert_eq!(cfg.persist_interval_ms, DEFAULT_PERSIST_MS);
    }

    #[test]
    fn out_of_range_values_are_fixed_with_warnings() {
        let (cfg, w) =
            parse("team_candidates = []\npersist_interval_ms = 1\nwatch_interval_ms = 999999\n")
                .expect("ok");
        assert_eq!(cfg.team_candidates, DEFAULT_CANDIDATES.to_vec());
        assert_eq!(cfg.persist_interval_ms, 10);
        assert_eq!(cfg.watch_interval_ms, 60_000);
        assert_eq!(w.len(), 3);
    }

    #[test]
    fn bad_types_and_values_are_errors() {
        assert!(parse("team_candidates = [300]").is_err()); // does not fit u8
        assert!(parse("team_candidates = \"six\"").is_err());
        assert!(parse("persist_interval_ms = -5").is_err());
        assert!(parse("this is not toml").is_err());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let (cfg, _) = parse("hostile_team_type = 6\n").expect("ok");
        assert_eq!(cfg, player_default());
    }

    #[test]
    fn dev_mode_defaults_to_false_and_parses() {
        assert!(!Config::default().dev_mode);
        let (cfg, _) = parse("").expect("ok");
        assert!(!cfg.dev_mode);
        let (cfg, _) = parse(
            "dev_mode = true
",
        )
        .expect("ok");
        assert!(cfg.dev_mode);
        assert!(parse("dev_mode = \"yes\"").is_err());
    }

    #[test]
    fn player_mode_forces_pvpve_on_with_rivals_and_logs_the_overrides() {
        let (cfg, w) = parse(
            "pvpve = false
pvpve_scheme = \"all\"
pvpve_team = 13
team_size = 2
",
        )
        .expect("ok");
        assert!(cfg.pvpve);
        assert_eq!(cfg.scheme(), Scheme::Rivals);
        assert_eq!((cfg.pvpve_team, cfg.team_size_usize()), (13, 2));
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[0].contains("pvpve = false") && w[1].contains("pvpve_scheme"));
        // Silent when the file does not ask for either.
        let (cfg, w) = parse(
            "pvpve_team = 29
",
        )
        .expect("ok");
        assert!(
            cfg.pvpve && cfg.scheme() == Scheme::Rivals && w.is_empty(),
            "{w:?}"
        );
        // The fallback used for a missing or broken file obeys the same rules.
        let f = fallback();
        assert!(f.pvpve && !f.dev_mode && f.pvpve_team == DEFAULT_PVPVE_TEAM);
    }

    /// R1: only a developer DLL (`dev` feature) honours `dev_mode = true`; a player DLL ignores it with a warning. The rest of this
    /// module's dev-mode tests run on the developer build only (`cargo test`); the player build is checked by `cargo clippy
    /// --no-default-features` plus this test under `cargo test --no-default-features -- dev_mode_needs`.
    #[test]
    fn removed_keys_are_ignored_with_one_warning() {
        let (cfg, w) = parse_dev(
            "overlay_enabled = true\nhotkey_mark = \"F10\"\nprobes = true\nteam_size = 2\n",
        )
        .expect("ok");
        assert_eq!(cfg.team_size, 2);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(
            w[0].contains("hotkey_mark")
                && w[0].contains("overlay_enabled")
                && w[0].contains("probes")
        );
    }

    #[test]
    fn dev_mode_needs_the_dev_build() {
        let (c, w) = parse("dev_mode = true\n").unwrap();
        assert_eq!(c.dev_mode, cfg!(feature = "dev"));
        assert_eq!(
            w.iter().any(|x| x.contains("player build")),
            !cfg!(feature = "dev")
        );
    }

    #[test]
    fn matchmaking_setting_drives_the_player_build_and_the_lobby_rules_parse() {
        // Default: on, with the documented lobby rules and no dev-only extras.
        let (cfg, w) = parse("").expect("ok");
        assert!(w.is_empty(), "{w:?}");
        assert!(cfg.matchmaking && cfg.mm_queue && cfg.mm_join && cfg.mm_quiet_invite);
        assert_eq!(
            (cfg.mm_max_players, cfg.mm_min_players, cfg.mm_fill_wait_s),
            (6, 2, 90)
        );
        assert!(!cfg.mm_probe && !cfg.mm_host_create && !cfg.mm_lobby_trace && !cfg.mm_sc_create);
        // Off: nothing runs.
        let (cfg, _) = parse("matchmaking = false\n").expect("ok");
        assert!(!cfg.mm_queue && !cfg.mm_join && !cfg.mm_quiet_invite);
        // Dev-only switches stay forced off whatever the file says.
        let (cfg, _) = parse(
            "mm_probe = true\nmm_host_create = true\nmm_lobby_trace = true\nmm_sc_create = true\n",
        )
        .expect("ok");
        assert!(
            cfg.mm_queue
                && !cfg.mm_probe
                && !cfg.mm_host_create
                && !cfg.mm_lobby_trace
                && !cfg.mm_sc_create
        );
        // Developer mode ignores `matchmaking`.
        let (cfg, _) = parse_dev("matchmaking = true\n").expect("ok");
        assert!(!cfg.mm_queue && !cfg.mm_join);
        // Lobby rules: ranges, min <= max.
        let (cfg, w) =
            parse("mm_max_players = 4\nmm_min_players = 3\nmm_fill_wait_s = 30\n").expect("ok");
        assert_eq!(
            (cfg.mm_max_players, cfg.mm_min_players, cfg.mm_fill_wait_s),
            (4, 3, 30)
        );
        assert!(w.is_empty(), "{w:?}");
        let (cfg, w) =
            parse("mm_max_players = 9\nmm_min_players = 1\nmm_fill_wait_s = 1\n").expect("ok");
        assert_eq!(
            (cfg.mm_max_players, cfg.mm_min_players, cfg.mm_fill_wait_s),
            (6, 2, 5)
        );
        assert_eq!(w.len(), 3, "{w:?}");
        let (cfg, w) = parse("mm_max_players = 3\nmm_min_players = 5\n").expect("ok");
        assert_eq!((cfg.mm_max_players, cfg.mm_min_players), (3, 3));
        assert_eq!(w.len(), 1, "{w:?}");
        // The older mm_match_players sets min = max (the mmtest zip, dev mode).
        let (cfg, w) = parse_dev("mm_queue = true\nmm_match_players = 2\n").expect("ok");
        assert_eq!((cfg.mm_max_players, cfg.mm_min_players), (2, 2));
        assert!(w.is_empty(), "{w:?}");
        let (cfg, w) = parse("mm_match_players = 3\nmm_max_players = 5\n").expect("ok");
        assert_eq!((cfg.mm_max_players, cfg.mm_min_players), (3, 3));
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(parse("matchmaking = \"yes\"").is_err());
    }

    #[test]
    fn dev_mode_keeps_the_config_as_written() {
        let (cfg, w) = parse(
            "dev_mode = true
pvpve = false
pvpve_scheme = \"all\"
",
        )
        .expect("ok");
        assert!(!cfg.pvpve && cfg.scheme() == Scheme::All);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn load_creates_the_default_file() {
        let dir = std::env::temp_dir().join(format!("nrpvp-cfg-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let cfg = load(&dir);
        assert_eq!(cfg, player_default());
        let written = std::fs::read_to_string(dir.join(FILE_NAME)).expect("file created");
        assert_eq!(written, DEFAULT_FILE_TEXT);
        // Second load reads it back.
        assert_eq!(load(&dir), player_default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn t039_rival_hit_seconds_default_parse_and_clamp() {
        assert_eq!(dev_default().rival_hit_seconds, 3.0);
        for dev in [true, false] {
            let pre = if dev { "dev_mode = true\n" } else { "" };
            let run = |t: &str| parse(&format!("{pre}{t}")).expect("ok");
            let (c, w) = run("rival_hit_seconds = 5.5\n");
            assert_eq!(c.rival_hit_seconds, 5.5);
            assert!(w.is_empty(), "{w:?}");
            let (c, w) = run("rival_hit_seconds = 4\n");
            assert_eq!(c.rival_hit_seconds, 4.0);
            assert!(w.is_empty(), "{w:?}");
            let (c, w) = run("rival_hit_seconds = 0.1\n");
            assert_eq!(c.rival_hit_seconds, 0.5);
            assert_eq!(w.len(), 1, "{w:?}");
            let (c, w) = run("rival_hit_seconds = 99.0\n");
            assert_eq!(c.rival_hit_seconds, 15.0);
            assert_eq!(w.len(), 1, "{w:?}");
            let (c, w) = run("rival_hit_seconds = nan\n");
            assert_eq!(c.rival_hit_seconds, 3.0);
            assert_eq!(w.len(), 1, "{w:?}");
        }
        assert!(parse("rival_hit_seconds = \"x\"").is_err());
    }

    #[test]
    fn hawk2_settings_default_on_parse_and_clamp() {
        assert_eq!(Config::default().hawk, HawkConfig::default());
        assert!(HawkConfig::default().enabled);
        // A player DLL refuses dev_mode with a warning, so the dev pass only runs in the dev build.
        for dev in [cfg!(feature = "dev"), false] {
            let pre = if dev { "dev_mode = true\n" } else { "" };
            let run = |t: &str| parse(&format!("{pre}{t}")).expect("ok");
            // No [hawk] table: on, with the HAWK-4 values.
            let (c, w) = run("team_size = 1\n");
            assert_eq!(c.hawk, HawkConfig::default());
            assert!(w.is_empty(), "{w:?}");
            let (c, w) = run("[hawk]\nenabled = false\ndive_secs = 4\n");
            assert!(!c.hawk.enabled);
            assert_eq!(c.hawk.dive_secs, 4.0);
            assert_eq!(c.hawk.dive_hold, 400);
            assert!(w.is_empty(), "{w:?}");
            let (c, w) = run("[hawk]\ndive_hold = 1\ndive_secs = 99\n");
            assert_eq!((c.hawk.dive_hold, c.hawk.dive_secs), (50, 15.0));
            assert_eq!(w.len(), 2, "{w:?}");
            let (c, w) = run("[hawk]\ndive_secs = nan\n");
            assert_eq!(c.hawk.dive_secs, 7.0);
            assert_eq!(w.len(), 1, "{w:?}");
        }
        assert!(parse("[hawk]\nenabled = \"yes\"\n").is_err());
        // A top-level key added below [hawk] belongs to [hawk]: it does not apply, and one warning says why.
        let (c, w) = parse("[hawk]\nenabled = true\nteam_size = 2\n").expect("ok");
        assert_eq!(c.team_size, 1);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("team_size"), "{w:?}");
    }

    #[test]
    fn t035_rival_revive_policy_and_removed_finisher_keys() {
        // Default: redown, in both modes.
        assert_eq!(dev_default().rival_revive, "redown");
        assert_eq!(dev_default().revive_policy(), RevivePolicy::Redown);
        assert_eq!(player_default().revive_policy(), RevivePolicy::Redown);
        // Dev mode honours "vanilla" (any case) and rejects other text with one warning.
        let (cfg, w) = parse_dev("rival_revive = \"Vanilla\"\n").expect("ok");
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(cfg.revive_policy(), RevivePolicy::Vanilla);
        let (cfg, w) = parse_dev("rival_revive = \"kill\"\n").expect("ok");
        assert_eq!(cfg.revive_policy(), RevivePolicy::Redown);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(parse("rival_revive = 3").is_err());
        // The player build ignores "vanilla" (with a line) and stays on redown.
        let (cfg, w) = parse("rival_revive = \"vanilla\"\n").expect("ok");
        assert_eq!(cfg.revive_policy(), RevivePolicy::Redown);
        assert_eq!(cfg.rival_revive, "redown");
        assert!(w.iter().any(|x| x.contains("vanilla")), "{w:?}");
        // The old finisher keys are ignored with exactly one line, in both modes, and no key is bound for them.
        for dev in [true, false] {
            let text = "finisher = \"hp0-all\"\nhotkey_finisher = \"F5\"\n";
            let (_, w) = if dev { parse_dev(text) } else { parse(text) }.expect("ok");
            let old: Vec<&String> = w.iter().filter(|x| x.contains("finisher")).collect();
            assert_eq!(old.len(), 1, "{w:?}");
        }
        let (_, w) = parse_dev("probes = false\n").expect("ok");
        assert!(w.iter().all(|x| !x.contains("finisher")), "{w:?}");
    }

    #[test]
    fn t032_snap_back_candidates() {
        let c = dev_default();
        assert_eq!(
            c.pos_candidates,
            vec![
                "Physics+0x70",
                "PlayerIns+0x6C0",
                "PlayerIns+0x6D0",
                "[PlayerIns+0x60]+0x1EC",
                "[PlayerIns+0x6B0]+0x17C",
                "[PlayerIns+0x6B0]+0x18C",
                "[PlayerIns+0x5F0]+0x64"
            ]
        );
        // Invalid entries are skipped with a warning; spacing and case are normalised.
        let (cfg, w) = parse_dev(
            "pos_candidates = [\"playerins + 0x10\", \"nonsense\", \"[PlayerIns+0x60]+0x1EC\", \"PlayerIns+0x10\"]
",
        )
        .expect("ok");
        assert_eq!(
            cfg.pos_candidates,
            vec!["PlayerIns+0x10", "[PlayerIns+0x60]+0x1EC"]
        );
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[0].contains("nonsense") && w[1].contains("duplicate"));
        // Nothing valid: the defaults, with a warning.
        let (cfg, w) = parse_dev("pos_candidates = [\"x\"]\n").expect("ok");
        assert_eq!(cfg.pos_candidates, dev_default().pos_candidates);
        assert_eq!(w.len(), 2, "{w:?}");
    }
}
