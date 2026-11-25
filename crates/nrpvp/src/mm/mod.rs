//! MM-2: public matchmaking queue (S9). Developer mode: switch `mm_queue = true`. Player build (MM-4): the player setting
//! `matchmaking` (default true) turns the queue, the join step and the quiet invite on; the overlay panel offers Solo and Leave.
//!
//! - [`machine`]: the pure queue state machine (`Idle -> Searching -> InQueue -> MatchReady`, plus `Leave`), written against
//!   the `QueueSteam` trait so it is unit-tested with a fake Steam.
//! - [`proto`]: the lobby data protocol (tags, roster text, ticket completeness).
//! - [`guard`]: the safety guard (Seamless Co-op loaded, no EasyAntiCheat module).
//! - [`worker`]: the glue that runs inside the Steam thread (`steam::probe`): real Steam, guard, hub commands, SC party tracking.
//!
//! The game loop and the overlay never call Steam. They only exchange plain values with the worker through this module's
//! globals: [`send`] (commands in), [`status`] (state out) and [`effective_team_size`] (the team-size override).

pub(crate) mod guard;
pub(crate) mod join;
pub(crate) mod machine;
pub(crate) mod pack;
pub(crate) mod proto;
pub(crate) mod sccreate;
pub(crate) mod ui_msg;
pub(crate) mod worker;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// Queue mode. The mode sets the team size of the match (S9 §6/§7): Solo 1, Duo 2, Squad 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Solo,
    Duo,
    Squad,
}

impl Mode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Mode::Solo => "solo",
            Mode::Duo => "duo",
            Mode::Squad => "squad",
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Mode> {
        match s {
            "solo" => Some(Mode::Solo),
            "duo" => Some(Mode::Duo),
            "squad" => Some(Mode::Squad),
            _ => None,
        }
    }
    /// Players per team for this mode (also the party size a ticket needs).
    pub(crate) fn team_size(self) -> usize {
        match self {
            Mode::Solo => 1,
            Mode::Duo => 2,
            Mode::Squad => 3,
        }
    }
}

/// What the overlay / hotkeys ask the worker to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cmd {
    Queue(Mode),
    /// ARENA-1: queue for an Arena match (lobby kind `arena`, two full teams).
    QueueArena(Mode),
    Leave,
    /// MM-3 (dev buttons): one join method attempt.
    Join(join::JoinCmd),
    /// MM-3d (dev button, `mm_sc_create`): create a lobby the way the game does and see whether Seamless Co-op adopts it.
    ScCreate,
    /// MM-3f (dev button, `mm_quiet_invite`): suppress the Steam invite window for 60 s so Invite Members can be tried solo.
    QuietTest,
}

/// What the overlay shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Status {
    /// One line, e.g. `Queue: solo 1/2 searching` or `MATCH READY host=...`.
    pub line: String,
    /// In some queue state other than Idle (the Leave button is enabled, the Queue buttons are not).
    pub active: bool,
    /// MM-3 (`mm_join`): show the join-method buttons.
    pub join_ui: bool,
    /// MM-3: the host's Seamless Co-op lobby id is known (buttons (a) (b) (c) usable).
    pub sc_known: bool,
    /// MM-3: we own that Seamless Co-op lobby (button (d) usable).
    pub sc_owner: bool,
    /// MM-3d (`mm_sc_create`): show the "create SC lobby (test)" button.
    pub sc_create_ui: bool,
    /// MM-3d: one status line of that test (empty until pressed).
    pub sc_create_line: String,
    /// MM-3f (`mm_quiet_invite`): show the "test quiet invite" button and hide the direct JoinLobby button.
    pub quiet_ui: bool,
    /// MM-3f: one status line of the suppression (empty when idle).
    pub quiet_line: String,
}

struct Hub {
    cmds: Mutex<VecDeque<Cmd>>,
    status: Mutex<Status>,
}

/// Commands queued but not yet taken; further sends are dropped (a stuck worker must not grow memory).
const MAX_CMDS: usize = 16;

static HUB: OnceLock<Hub> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
/// MM-3g + MM-7: `mm_auto_start` is on.
static AUTO_START: AtomicBool = AtomicBool::new(false);
/// 0 = no override.
static TEAM_OVERRIDE: AtomicUsize = AtomicUsize::new(0);
/// MM-3: the local player's team_type as last read by the game loop (0 = never read). Only written while the feature is enabled.
static LOCAL_TEAM: AtomicU8 = AtomicU8::new(0);
/// MM-4: when the game loop last stored the local team (milliseconds since [`epoch`], plus 1; 0 = never).
static LOCAL_AT_MS: AtomicU64 = AtomicU64::new(0);
/// MM-3f: how many players the local ChrSet held at the game loop's last read (0 = never read).
static LOCAL_PLAYERS: AtomicUsize = AtomicUsize::new(0);
/// MM-FILL part B: the local player's session id (`ChrIns+0x78`) as the game loop last read it (0 = unknown). Only written while enabled.
static LOCAL_PID: AtomicU64 = AtomicU64::new(0);
/// MM-FILL part B: session player id -> roster team, built by the worker while MATCH READY (`None` outside a match).
static ROSTER_TEAMS: Mutex<Option<std::collections::HashMap<u64, u8>>> = Mutex::new(None);
/// UI-1: the queue phase, written by the worker every tick, read lock-free (also from the Commence exception handler).
static PHASE: AtomicU8 = AtomicU8::new(PHASE_IDLE);
pub(crate) const PHASE_IDLE: u8 = 0;
/// Searching, creating, joining or in a queue lobby.
pub(crate) const PHASE_SEARCHING: u8 = 1;
/// MATCH READY (the match exists; the host starts the expedition).
pub(crate) const PHASE_MATCHED: u8 = 2;

/// ARENA-1: a match found by an Arena queue is on (written by the worker every tick).
static ARENA_MATCH: AtomicBool = AtomicBool::new(false);

/// Worker (ARENA-1): publish whether the current match is an Arena match.
pub(crate) fn set_arena_match(on: bool) {
    ARENA_MATCH.store(on, Ordering::Relaxed);
}

/// ARENA-4: the next (or current) expedition is an offline Arena match against bots (Bots On + Arena card + Commence; set by the
/// Commence handler, cleared by the bots feature when that expedition is over). ARENA-6: the value is the team size (1 Solo: LOCAL vs
/// one bot; 2 Duo: LOCAL + 1 bot ally vs 2 bots; 3 Trio: LOCAL + 2 bot allies vs 3 bots); 0 = none.
static ARENA_BOTS: AtomicU8 = AtomicU8::new(0);

/// ARENA-4 / ARENA-6: start an Arena match against bots with this team size (1..=3), or forget it (0).
pub(crate) fn set_arena_bots(size: u8) {
    let size = size.min(3);
    let was = ARENA_BOTS.swap(size, Ordering::Relaxed);
    if was != size {
        if size > 0 {
            tracing::info!(
                "arena: match against bots requested (Bots On, Arena card, Commence): {size} vs {size}, {} bot(s)",
                2 * size - 1
            );
        } else {
            tracing::info!("arena: match against bots over");
        }
    }
}

/// ARENA-4: the current expedition is an Arena match against bots (no queue, no session).
pub(crate) fn arena_bots() -> bool {
    ARENA_BOTS.load(Ordering::Relaxed) > 0
}

/// ARENA-6: the team size of the current Arena match against bots (0 = none).
pub(crate) fn arena_bots_size() -> u8 {
    ARENA_BOTS.load(Ordering::Relaxed)
}

/// Game loop (ARENA-1): the current match is an Arena match: one found by the Arena queue (false when the queue is off), or an
/// offline match against a bot (ARENA-4).
pub(crate) fn arena_match() -> bool {
    (enabled() && ARENA_MATCH.load(Ordering::Relaxed)) || arena_bots()
}

/// Game loop (ARENA-1): LOCAL's roster team (0-based) in the current match, from the session id map (`None` until it is known).
pub(crate) fn local_roster_team() -> Option<u8> {
    let pid = local_pid()?;
    roster_teams()?.get(&pid).copied()
}

/// Game loop (ARENA-5): LOCAL's place in its roster team (0-based, by session id; the same on every game), `None` until it is known.
pub(crate) fn local_roster_slot() -> Option<u8> {
    roster_slot(&roster_teams()?, local_pid()?)
}

/// ARENA-5 (pure): `pid`'s place among the players of its team in `map`, by session id.
pub(crate) fn roster_slot(map: &std::collections::HashMap<u64, u8>, pid: u64) -> Option<u8> {
    let team = *map.get(&pid)?;
    let below = map.iter().filter(|(p, t)| **t == team && **p < pid).count();
    u8::try_from(below).ok()
}

/// Worker: publish the queue phase.
pub(crate) fn set_phase(p: u8) {
    PHASE.store(p, Ordering::Relaxed);
}

/// The queue phase ([`PHASE_IDLE`] when the feature is off).
pub(crate) fn phase() -> u8 {
    if enabled() {
        PHASE.load(Ordering::Relaxed)
    } else {
        PHASE_IDLE
    }
}

fn hub() -> &'static Hub {
    HUB.get_or_init(|| Hub {
        cmds: Mutex::new(VecDeque::new()),
        status: Mutex::new(Status::default()),
    })
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Called once at startup when `dev_mode && mm_queue`. Until then every function below is inert.
pub(crate) fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
}

/// MM-3g + MM-7: a found match starts by itself.
pub(crate) fn auto_start() -> bool {
    AUTO_START.load(Ordering::Relaxed)
}

pub(crate) fn set_auto_start(on: bool) {
    AUTO_START.store(on, Ordering::Relaxed);
}

pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Queue a command for the worker. `false` when the queue feature is off or the queue is full.
pub(crate) fn send(c: Cmd) -> bool {
    if !enabled() {
        return false;
    }
    let mut q = lock(&hub().cmds);
    if q.len() >= MAX_CMDS {
        return false;
    }
    q.push_back(c);
    true
}

/// Worker: take everything queued.
pub(crate) fn take_cmds() -> Vec<Cmd> {
    lock(&hub().cmds).drain(..).collect()
}

/// Worker: publish the state for the overlay.
pub(crate) fn publish(s: Status) {
    *lock(&hub().status) = s;
}

/// Overlay: the latest published state.
pub(crate) fn status() -> Status {
    lock(&hub().status).clone()
}

/// Game loop (MM-FILL part B): the local player's session id, `None` when unreadable. Logs every change.
pub(crate) fn set_local_pid(pid: Option<u64>) {
    if !enabled() {
        return;
    }
    let new = pid.unwrap_or(0);
    let old = LOCAL_PID.swap(new, Ordering::Relaxed);
    if old != new {
        tracing::info!("mm_team: local pid 0x{new:X} (changed from 0x{old:X})");
    }
}

/// Worker (MM-FILL part B): the local session id the game loop stored, `None` while unknown.
pub(crate) fn local_pid() -> Option<u64> {
    Some(LOCAL_PID.load(Ordering::Relaxed)).filter(|&p| p != 0)
}

/// Worker (MM-FILL part B): publish (or clear) the session id -> roster team map.
pub(crate) fn set_roster_teams(map: Option<std::collections::HashMap<u64, u8>>) {
    *lock(&ROSTER_TEAMS) = map;
}

/// Game loop (MM-FILL part B): session player id -> team from the queue roster; `None` outside a match.
pub(crate) fn roster_teams() -> Option<std::collections::HashMap<u64, u8>> {
    if !enabled() {
        return None;
    }
    lock(&ROSTER_TEAMS).clone()
}

/// Worker: set (`Some(n)`) or clear (`None`) the team-size override. Set on `MatchReady`, cleared on leave / back to Idle.
pub(crate) fn set_team_override(n: Option<usize>) {
    TEAM_OVERRIDE.store(n.unwrap_or(0), Ordering::SeqCst);
}

/// Game loop: the local player's current `team_type` (read by `TeamCtl::watch_tick`). Inert unless the matchmaking feature is on.
pub(crate) fn note_local_team(team: u8) {
    if enabled() {
        LOCAL_TEAM.store(team, Ordering::Relaxed);
        let ms = u64::try_from(epoch().elapsed().as_millis()).unwrap_or(u64::MAX - 1);
        LOCAL_AT_MS.store(ms.saturating_add(1), Ordering::Relaxed);
    }
}

/// Game loop: the number of players in the local ChrSet (read by the gate's ALL PRESENT check). Inert unless mm is enabled.
pub(crate) fn note_local_players(n: usize) {
    if enabled() {
        LOCAL_PLAYERS.store(n, Ordering::Relaxed);
    }
}

/// Worker: the last ChrSet player count the game loop stored (0 = never read).
pub(crate) fn local_players() -> usize {
    LOCAL_PLAYERS.load(Ordering::Relaxed)
}

/// FS7-8: a defeated player chose to leave the match; the worker leaves the Seamless Co-op lobby once back at the Roundtable.
static LEAVE_SESSION: AtomicBool = AtomicBool::new(false);
/// FS7-4: the own party's size as the worker last saw it (0 = unknown): the Seamless Co-op lobby's members, or the size of the own party
/// of the last match while still in that match's lobby (strangers of a Solo match are no party).
static PARTY_SIZE: AtomicUsize = AtomicUsize::new(0);

/// Match rules (FS7-8): ask the worker to leave the Seamless Co-op lobby when the player is back at the Roundtable.
pub(crate) fn ask_leave_session() {
    if enabled() {
        LEAVE_SESSION.store(true, Ordering::Relaxed);
    }
}

/// Worker (FS7-8): whether a leave was asked; `take` clears it.
pub(crate) fn leave_session_asked(take: bool) -> bool {
    if take {
        LEAVE_SESSION.swap(false, Ordering::Relaxed)
    } else {
        LEAVE_SESSION.load(Ordering::Relaxed)
    }
}

/// Worker (FS7-4): publish the own party's size.
pub(crate) fn set_party_size(n: usize) {
    PARTY_SIZE.store(n, Ordering::Relaxed);
}

/// Menu (FS7-4): the own party's size (`None`: the queue is off or the worker has not looked yet).
pub(crate) fn party_size() -> Option<usize> {
    match PARTY_SIZE.load(Ordering::Relaxed) {
        0 => None,
        n if enabled() => Some(n),
        _ => None,
    }
}

/// FS7-4 / MM-FILL: the last match as the worker remembers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LastMatch {
    /// The match's Seamless Co-op lobby.
    pub sc: u64,
    /// The mode's team size (1 Solo, 2 Duo, 3 Trio).
    pub team: usize,
    /// The own ticket's party size (MM-FILL: 1 for a player whose teammates were random).
    pub party: usize,
    /// ARENA-2: the match came from an Arena queue.
    pub arena: bool,
    /// ARENA-2: a private match (room code): the players arranged it together.
    pub private: bool,
}

/// FS7-4 (pure): the own party's size: the Seamless Co-op lobby's member count, except in the lobby of the last match, where the
/// other members are that match's rivals and the own party is the match's team size. No lobby = alone.
pub(crate) fn party_of(sc: Option<(u64, u32)>, last_match: Option<(u64, usize)>) -> usize {
    match (sc, last_match) {
        (None, _) => 1,
        (Some((id, _)), Some((m, size))) if id == m => size,
        (Some((_, members)), _) => (members as usize).max(1),
    }
}

/// MM-FILL part C (pure): leave the last match's Seamless Co-op lobby once back at the Roundtable after a Duo / Trio match the
/// player queued for alone. Its other members are random teammates and rivals; staying would make them follow the lobby owner's
/// next queue (`sync_party`) as if they were a party. Solo keeps today's behaviour (a Solo ticket never publishes `nrpvp_q`), and a
/// premade party stays together. ARENA-2: after a public Arena match every player leaves too (its rival is a random player; Leonardo
/// 2026-11-08: only friends who arranged the match stay together, here a private match).
pub(crate) fn leave_after_fill(
    at_roundtable: bool,
    in_match: bool,
    sc: Option<u64>,
    last: Option<LastMatch>,
) -> bool {
    at_roundtable
        && !in_match
        && matches!((sc, last), (Some(id), Some(m)) if id == m.sc
            && ((m.team > 1 && m.party == 1) || (m.arena && !m.private)))
}

#[cfg(test)]
mod party_tests {
    use super::{leave_after_fill, party_of, LastMatch};

    #[test]
    fn a_lone_duo_or_trio_player_leaves_the_match_lobby_back_at_the_roundtable() {
        let lone_duo = LastMatch {
            sc: 7,
            team: 2,
            party: 1,
            arena: false,
            private: false,
        };
        assert!(leave_after_fill(true, false, Some(7), Some(lone_duo)));
        assert!(
            !leave_after_fill(true, true, Some(7), Some(lone_duo)),
            "not while the match is on"
        );
        assert!(
            !leave_after_fill(false, false, Some(7), Some(lone_duo)),
            "only at the Roundtable"
        );
        assert!(
            !leave_after_fill(true, false, Some(9), Some(lone_duo)),
            "a new lobby (an invite after the match) is kept"
        );
        assert!(!leave_after_fill(true, false, None, Some(lone_duo)));
        assert!(!leave_after_fill(true, false, Some(7), None));
        let pair = LastMatch {
            party: 2,
            ..lone_duo
        };
        assert!(
            !leave_after_fill(true, false, Some(7), Some(pair)),
            "a premade pair stays"
        );
        let solo = LastMatch {
            team: 1,
            ..lone_duo
        };
        assert!(
            !leave_after_fill(true, false, Some(7), Some(solo)),
            "Solo unchanged"
        );
        let arena = LastMatch {
            arena: true,
            ..solo
        };
        assert!(
            leave_after_fill(true, false, Some(7), Some(arena)),
            "a public Arena match: the rival was random"
        );
        assert!(
            !leave_after_fill(true, true, Some(7), Some(arena)),
            "not while the Arena match is on"
        );
        let friends = LastMatch {
            private: true,
            ..arena
        };
        assert!(
            !leave_after_fill(true, false, Some(7), Some(friends)),
            "a private Arena match stays together"
        );
    }

    #[test]
    fn the_party_is_the_lobby_except_in_the_last_match_lobby() {
        assert_eq!(party_of(None, None), 1, "no lobby: alone");
        assert_eq!(
            party_of(Some((7, 2)), None),
            2,
            "an Invite Members party of two"
        );
        assert_eq!(
            party_of(Some((7, 3)), Some((7, 1))),
            1,
            "back from a Solo match with two rivals in its lobby"
        );
        assert_eq!(
            party_of(Some((7, 3)), Some((7, 2))),
            2,
            "a Duo match: the own pair"
        );
        assert_eq!(
            party_of(Some((9, 3)), Some((7, 1))),
            3,
            "a new lobby after the match"
        );
        assert_eq!(party_of(Some((9, 0)), None), 1);
    }
}

/// Worker: the local player's phase according to the last `team_type` the game loop read (`None` = never read / not 1 or 2).
pub(crate) fn local_phase() -> Option<join::Phase> {
    join::Phase::from_team(LOCAL_TEAM.load(Ordering::Relaxed))
}

fn epoch() -> Instant {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    *EPOCH.get_or_init(Instant::now)
}

/// MM-4: where the local player is, as far as the game loop's local-player read can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum World {
    /// The game loop never found a local player (title screen, or still loading).
    Never,
    /// It found one before, but not for a while: the world was unloaded (quit to the title, or a loading screen).
    Gone,
    /// A local player was read recently, with this phase (`None` = a `team_type` that tells nothing).
    Present(Option<join::Phase>),
}

/// Pure rule behind [`world_state`]: `age` is the time since the last local-player read (`None` = never).
pub(crate) fn classify_world(team: u8, age: Option<Duration>, stale_after: Duration) -> World {
    match age {
        None => World::Never,
        Some(a) if a > stale_after => World::Gone,
        Some(_) => World::Present(join::Phase::from_team(team)),
    }
}

/// Worker: the local player's world state now. `stale_after` should be a few watcher periods (the game loop reads once per period).
pub(crate) fn world_state(stale_after: Duration) -> World {
    let at = LOCAL_AT_MS.load(Ordering::Relaxed);
    let age = at
        .checked_sub(1)
        .map(|ms| epoch().elapsed().saturating_sub(Duration::from_millis(ms)));
    classify_world(LOCAL_TEAM.load(Ordering::Relaxed), age, stale_after)
}

/// The team size the rules must use: the match override when one is set, else the configured value. With `mm_queue` off
/// nothing ever sets the override, so this returns `configured` unchanged.
pub(crate) fn effective_team_size(configured: usize) -> usize {
    pick_team_size(configured, TEAM_OVERRIDE.load(Ordering::SeqCst))
}

pub(crate) fn pick_team_size(configured: usize, override_value: usize) -> usize {
    if override_value == 0 {
        configured
    } else {
        override_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_map_to_team_sizes_and_labels() {
        assert_eq!(
            [Mode::Solo, Mode::Duo, Mode::Squad].map(|m| (m.label(), m.team_size())),
            [("solo", 1), ("duo", 2), ("squad", 3)]
        );
        for m in [Mode::Solo, Mode::Duo, Mode::Squad] {
            assert_eq!(Mode::parse(m.label()), Some(m));
        }
        assert_eq!(Mode::parse("trio"), None);
    }

    #[test]
    fn world_states_from_the_last_local_read() {
        use super::join::Phase;
        let stale = Duration::from_secs(4);
        assert_eq!(classify_world(1, None, stale), World::Never);
        assert_eq!(
            classify_world(1, Some(Duration::from_secs(1)), stale),
            World::Present(Some(Phase::Roundtable))
        );
        assert_eq!(
            classify_world(2, Some(Duration::from_secs(1)), stale),
            World::Present(Some(Phase::Expedition))
        );
        assert_eq!(
            classify_world(29, Some(Duration::from_secs(1)), stale),
            World::Present(None)
        );
        assert_eq!(
            classify_world(1, Some(Duration::from_secs(5)), stale),
            World::Gone,
            "no read for longer than the limit"
        );
        assert_eq!(
            classify_world(1, Some(stale), stale),
            World::Present(Some(Phase::Roundtable))
        );
    }

    #[test]
    fn a_players_slot_is_its_rank_by_session_id_inside_its_team() {
        let map: std::collections::HashMap<u64, u8> = [(30, 1), (10, 0), (20, 1), (40, 0), (50, 1)]
            .into_iter()
            .collect();
        assert_eq!(roster_slot(&map, 10), Some(0));
        assert_eq!(roster_slot(&map, 40), Some(1));
        assert_eq!(roster_slot(&map, 20), Some(0));
        assert_eq!(roster_slot(&map, 30), Some(1));
        assert_eq!(roster_slot(&map, 50), Some(2));
        assert_eq!(roster_slot(&map, 99), None, "not in the roster");
    }

    #[test]
    fn team_size_override_applies_only_when_set() {
        assert_eq!(
            pick_team_size(3, 0),
            3,
            "no override: the configured value, unchanged"
        );
        assert_eq!(pick_team_size(1, 2), 2);
        assert_eq!(pick_team_size(3, 1), 1);
    }

    #[test]
    fn global_override_is_zero_until_set() {
        // Not set here: the global is shared with the rest of the test binary (team rule tests), so only the default is checked.
        assert_eq!(effective_team_size(2), 2);
    }

    #[test]
    fn commands_do_nothing_until_enabled_and_are_bounded() {
        // `enable` is process-global, so this is the only test that calls it.
        assert!(!enabled());
        assert!(!send(Cmd::Leave));
        enable();
        assert!(take_cmds().is_empty());
        assert!(send(Cmd::Queue(Mode::Solo)));
        assert!(send(Cmd::Leave));
        assert_eq!(take_cmds(), vec![Cmd::Queue(Mode::Solo), Cmd::Leave]);
        for _ in 0..MAX_CMDS {
            assert!(send(Cmd::Leave));
        }
        assert!(!send(Cmd::Leave));
        assert_eq!(take_cmds().len(), MAX_CMDS);
        publish(Status {
            line: "x".into(),
            active: true,
            ..Default::default()
        });
        assert_eq!(
            status(),
            Status {
                line: "x".into(),
                active: true,
                ..Default::default()
            }
        );
    }
}
