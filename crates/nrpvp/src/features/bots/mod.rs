//! Bots (BOTS.md). OFF-4 started the game's own NPC bots (AI Nightfarers) by hand; BOT-1 brings back the fill director ([`fill`]):
//! an offline expedition is filled with `bots_fill` hostile bots and a roster follows them. The game's bot manager is also the engine
//! of the orchestrator's puppet players (`game/puppet.rs`, `tools/`). Research and the engine facts: `archive/research/OFF-3-bots.md`,
//! TECHNICAL_NOTES §4d, MEMORY_MAP §15 / §18.
//!
//! No game code is called from here. Our events in `event/common.emevd.dcx` (`gamedata/emevd-bots.json`, built by
//! `scripts/build-gamedata.ps1`) wait for our flags and run the game's instructions: `2003[97](table)` arms `CSNpcBotMan` with an
//! `NPCBotTableParam` table (the manager then spawns its Nightfarers), `2003[98]` removes them. This feature sets the flags
//! (through [`Ctx::flags`]), asks for the data writes of the fill ([`super::BotWrite`]) and, every tick, logs a change of the manager.
//!
//! Offline mode only for now: the game itself refuses `2003[97]` when its solo gate counts more than one (friends + bots: BOT-0).
//! Requests come from the config (`bots_fill`), the dev console (`bots`, `bots_clear`, `bots_fill`, `bots_list`) and the puppets.
//! Log lines start with `bots:`.

pub(crate) mod fill;

/// BOT-5: the next expedition is a match against bots (set by the Commence handler, `game::menuflow`, when Bots is On; cleared when that
/// expedition is over). Read every tick by the fill director.
static BOTS_MATCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// BOT-5: start (or forget) a match against bots for the next expedition.
pub(crate) fn set_bots_match(on: bool) {
    if BOTS_MATCH.swap(on, std::sync::atomic::Ordering::Relaxed) != on {
        tracing::info!(
            "bots: match against bots {}",
            if on {
                "requested (Bots On, Commence)"
            } else {
                "over"
            }
        );
    }
}

/// BOT-2: bots killed by a mob whose replacement has not spawned yet (written by the fill director every tick).
static PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub(crate) fn pending() -> usize {
    PENDING.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn bots_match() -> bool {
    BOTS_MATCH.load(std::sync::atomic::Ordering::Relaxed)
}

use std::sync::{Mutex, PoisonError};

use super::mode::GameMode;
use super::{Ctx, Feature};
use crate::game::views::{BotManState, BOT_HEADS};
use crate::game::world::World;

/// A request for the bot manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BotsCmd {
    /// Arm this `NPCBotTableParam` table (`2003[97]`).
    Spawn(i32),
    /// Remove every bot (`2003[98]`).
    Clear,
    /// BOT-1: fill the expedition with this many hostile bots (0..=5).
    Fill(usize),
}

/// Tables our events know, with their flags (`gamedata/emevd-bots.json`; a unit test checks both files). 10 / 30 / 50: 2
/// Nightfarers at level 1 (heroes 1 + 2, 3 + 4, 5 + 6); 10000: 2 of the 8 base Nightfarers at level 6 (the manager holds 2).
pub(crate) const TABLES: [(i32, u32); 5] = [
    (10, 10_007_300),
    (30, 10_007_303),
    (50, 10_007_304),
    (10_000, 10_007_301),
    fill::ARENA_TABLE,
];
/// Flag of the `2003[98]` event.
pub(crate) const FLAG_CLEAR: u32 = 10_007_302;

/// The flag that carries out `cmd`, or why it is refused (pure; unit tested).
pub(crate) fn plan(cmd: BotsCmd, mode: GameMode, world: &World) -> Result<u32, String> {
    // BOT-0: the console may ask in any mode (the game's own solo gate still refuses in a session: `bots_gate`).
    let _ = mode;
    if world.wcm_instance == 0 {
        return Err("no world loaded".into());
    }
    match cmd {
        BotsCmd::Spawn(t) => TABLES
            .iter()
            .find(|(id, _)| *id == t)
            .map(|(_, f)| *f)
            .ok_or_else(|| {
                format!(
                    "no event for table {t}; tables: {}",
                    TABLES.map(|(id, _)| id.to_string()).join(", ")
                )
            }),
        BotsCmd::Clear => Ok(FLAG_CLEAR),
        BotsCmd::Fill(_) => Err("fill is not a flag request".into()),
    }
}

/// The manager's state in words (pure).
fn describe(s: Option<&BotManState>) -> String {
    match s {
        None => "no bot manager".into(),
        Some(s) => {
            let mut l = format!(
                "table {}, {}, {} bot(s)",
                s.table,
                if s.armed { "armed" } else { "idle" },
                s.bots
            );
            let shown = s.bots.min(BOT_HEADS);
            if shown > 0 {
                let heads: Vec<String> = s.heads[..shown]
                    .iter()
                    .map(|h| format!("0x{h:X}"))
                    .collect();
                l.push_str(&format!(" [{}]", heads.join(", ")));
            }
            l
        }
    }
}

/// Last request result and the manager's state, for the console (written by the main loop, read by the pipe thread).
static STATUS: Mutex<(String, String)> = Mutex::new((String::new(), String::new()));

/// `bots: <last request>; manager: <state>` (console).
pub(crate) fn status_line() -> String {
    let s = STATUS.lock().unwrap_or_else(PoisonError::into_inner);
    format!(
        "bots: {}; manager: {}",
        if s.0.is_empty() {
            "no request yet"
        } else {
            &s.0
        },
        if s.1.is_empty() { "not read yet" } else { &s.1 }
    )
}

fn set_status(request: Option<String>, manager: Option<String>) {
    let mut s = STATUS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(r) = request {
        s.0 = r;
    }
    if let Some(m) = manager {
        s.1 = m;
    }
}

/// BOT-1: the roster text for the console (`bots_list`; written by the main loop every tick, read by the pipe thread).
static ROSTER: Mutex<String> = Mutex::new(String::new());

/// `bots_list` (console): the fill director's roster.
pub(crate) fn roster_line() -> String {
    let s = ROSTER.lock().unwrap_or_else(PoisonError::into_inner);
    if s.is_empty() {
        "bots: no roster yet".into()
    } else {
        s.clone()
    }
}

pub(crate) struct BotsFeature {
    /// The manager's state last logged (`None` before the first read).
    last: Option<Option<BotManState>>,
    /// BOT-1: the fill director and its roster.
    pub(crate) fill: fill::BotFill,
    epoch: std::time::Instant,
    /// ARENA-4: how far an Arena match against a bot has come (the switch is cleared when it is over).
    arena_track: ArenaTrack,
}

/// ARENA-4: how far an Arena match against a bot has come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ArenaTrack {
    /// Commence pressed, the expedition not reached yet.
    #[default]
    Before,
    /// In the expedition, before the arrival in the arena.
    Expedition,
    /// LOCAL stands in the arena; the `WorldChrMan` instance there.
    Arena(usize),
}

/// What the Arena looks like this tick for [`arena_bots_over`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArenaSeen {
    pub on: bool,
    /// The warp was asked for (`arenacard::warped`).
    pub warped: bool,
    /// LOCAL stands on its spawn (`arenacard::placed`).
    pub placed: bool,
    pub wcm: usize,
}

/// ARENA-4 (pure): whether an Arena match against a bot is over. In the arena an offline LOCAL reads the Roundtable's character type,
/// so the end is the next map load after the arrival (the ending's way back, or a quit), not the place. Before the warp, back at the
/// Roundtable is the end (a quit from the expedition). Returns the new track and whether to clear the switch.
pub(crate) fn arena_bots_over(t: ArenaTrack, s: ArenaSeen, world: &World) -> (ArenaTrack, bool) {
    use crate::game::views::Place;
    if !s.on {
        return (ArenaTrack::Before, false);
    }
    match t {
        ArenaTrack::Before if world.in_expedition() => (ArenaTrack::Expedition, false),
        ArenaTrack::Expedition if s.placed && s.wcm != 0 => (ArenaTrack::Arena(s.wcm), false),
        ArenaTrack::Expedition if !s.warped && world.place == Some(Place::Roundtable) => {
            (ArenaTrack::Before, true)
        }
        ArenaTrack::Arena(w) if s.wcm != w => (ArenaTrack::Before, true),
        t => (t, false),
    }
}

impl Default for BotsFeature {
    fn default() -> Self {
        Self::new(0, 29)
    }
}

impl BotsFeature {
    /// `want` = `bots_fill` (0 = no fill until the console asks), `rival_team` = `pvpve_team`.
    pub(crate) fn new(want: usize, rival_team: u8) -> Self {
        BotsFeature {
            last: None,
            fill: fill::BotFill::new(want, rival_team),
            epoch: std::time::Instant::now(),
            arena_track: ArenaTrack::Before,
        }
    }

    fn now(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Ask for `cmd`: sets our flag when allowed (the flag is written after every feature ticked).
    pub(crate) fn request(
        &mut self,
        cmd: BotsCmd,
        src: &str,
        mode: GameMode,
        world: &World,
        ctx: &mut Ctx,
    ) {
        if let BotsCmd::Fill(n) = cmd {
            self.fill.set_want(n, self.now());
            let line = format!("fill {} asked by {src}", self.fill.want());
            tracing::info!("bots: {line}");
            set_status(Some(line), None);
            return;
        }
        let line = match plan(cmd, mode, world) {
            Ok(flag) => {
                ctx.flags.push((flag, true));
                format!("{cmd:?} asked by {src} (flag {flag})")
            }
            Err(e) => format!("{cmd:?} refused: {e}"),
        };
        tracing::info!("bots: {line}");
        set_status(Some(line), None);
    }
}

impl Feature for BotsFeature {
    fn name(&self) -> &'static str {
        "bots"
    }

    fn tick(&mut self, world: &World, ctx: &mut Ctx) {
        let now = self.now();
        let arena_on = crate::mm::arena_bots();
        // ARENA-4: the Arena (its card, warp and rounds); every build has it since 2026-11-13.
        let (warped, placed) = (
            crate::game::arenacard::warped(),
            crate::game::arenacard::placed(),
        );
        let seen = ArenaSeen {
            on: arena_on,
            warped,
            placed,
            wcm: world.wcm_instance,
        };
        let (track, over) = arena_bots_over(self.arena_track, seen, world);
        if track != self.arena_track {
            tracing::info!(
                "arena: match against a bot: {:?} -> {track:?}",
                self.arena_track
            );
        }
        self.arena_track = track;
        if over {
            crate::mm::set_arena_bots(0);
        }
        self.fill.set_bots_match(bots_match());
        self.fill.set_arena(fill::ArenaIn {
            on: arena_on && !over,
            size: crate::mm::arena_bots_size(),
            placed,
            events: crate::game::arenarounds::bot_events(),
        });
        self.fill.step(now, super::mode::current(), world, ctx);
        crate::game::arenarounds::set_arena_bots(
            self.fill.arena_bots(),
            self.fill.arena_all_ready(),
        );
        PENDING.store(self.fill.pending(), std::sync::atomic::Ordering::Relaxed);
        if self.fill.take_over() {
            set_bots_match(false);
        }
        *ROSTER.lock().unwrap_or_else(PoisonError::into_inner) = self.fill.roster();
        if self.last == Some(world.bots) {
            return;
        }
        let line = describe(world.bots.as_ref());
        // The first read and the manager's own changes are evidence (OFF-4); a vanished manager is an area load.
        tracing::info!("bots: CSNpcBotMan {line}");
        set_status(None, Some(line));
        self.last = Some(world.bots);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live() -> World {
        World {
            wcm_instance: 0x5_0000,
            ..World::default()
        }
    }

    #[test]
    fn requests_need_a_world_in_any_mode() {
        assert_eq!(
            plan(BotsCmd::Spawn(10), GameMode::BattleRoyale, &live()),
            Ok(10_007_300)
        );
        assert!(plan(BotsCmd::Clear, GameMode::Offline, &World::default())
            .unwrap_err()
            .contains("no world"));
    }

    #[test]
    fn known_tables_map_to_their_flags() {
        assert_eq!(
            plan(BotsCmd::Spawn(10), GameMode::Offline, &live()),
            Ok(10_007_300)
        );
        assert_eq!(
            plan(BotsCmd::Spawn(10_000), GameMode::Offline, &live()),
            Ok(10_007_301)
        );
        assert_eq!(
            plan(BotsCmd::Clear, GameMode::Offline, &live()),
            Ok(FLAG_CLEAR)
        );
        assert!(plan(BotsCmd::Spawn(20), GameMode::Offline, &live())
            .unwrap_err()
            .contains("tables: 10, 30, 50, 10000, 30000"));
    }

    #[test]
    fn a_request_queues_the_flag_only_when_allowed() {
        let mut f = BotsFeature::default();
        let mut ctx = Ctx::default();
        f.request(
            BotsCmd::Spawn(10),
            "test",
            GameMode::Offline,
            &live(),
            &mut ctx,
        );
        assert_eq!(ctx.flags, vec![(10_007_300, true)]);
        let mut ctx = Ctx::default();
        f.request(
            BotsCmd::Clear,
            "test",
            GameMode::BattleRoyale,
            &World::default(),
            &mut ctx,
        );
        assert!(ctx.flags.is_empty());
    }

    #[test]
    fn an_arena_bots_match_is_over_at_the_first_map_load_after_the_arrival() {
        use crate::game::views::Place;
        use ArenaTrack::*;
        let at = |p| World {
            place: Some(p),
            ..World::default()
        };
        let seen = |warped, placed, wcm| ArenaSeen {
            on: true,
            warped,
            placed,
            wcm,
        };
        let rt = at(Place::Roundtable);
        let ex = at(Place::Expedition);
        assert_eq!(
            arena_bots_over(Arena(5), ArenaSeen::default(), &rt),
            (Before, false),
            "off"
        );
        assert_eq!(
            arena_bots_over(Before, seen(false, false, 1), &rt),
            (Before, false),
            "Commence pressed, still at the table"
        );
        assert_eq!(
            arena_bots_over(Before, seen(false, false, 0), &World::default()),
            (Before, false),
            "loading"
        );
        assert_eq!(
            arena_bots_over(Before, seen(false, false, 2), &ex),
            (Expedition, false)
        );
        // The warp: the arena reads the Roundtable's type and a new world; not the end.
        assert_eq!(
            arena_bots_over(Expedition, seen(true, false, 3), &rt),
            (Expedition, false),
            "the warp's load and the arrival"
        );
        assert_eq!(
            arena_bots_over(Expedition, seen(true, true, 3), &rt),
            (Arena(3), false)
        );
        assert_eq!(
            arena_bots_over(Arena(3), seen(true, true, 3), &rt),
            (Arena(3), false),
            "in the arena"
        );
        assert_eq!(
            arena_bots_over(Arena(3), seen(true, true, 0), &World::default()),
            (Before, true),
            "the ending's load"
        );
        // A quit from the expedition before the warp.
        assert_eq!(
            arena_bots_over(Expedition, seen(false, false, 4), &rt),
            (Before, true)
        );
    }

    #[test]
    fn manager_state_reads_as_one_line() {
        assert_eq!(describe(None), "no bot manager");
        let s = BotManState {
            table: 10,
            armed: true,
            bots: 2,
            heads: [0xA, 0xB, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(describe(Some(&s)), "table 10, armed, 2 bot(s) [0xA, 0xB]");
    }

    /// Every table and the clear flag have their event in gamedata/emevd-bots.json (plain-text check: one event per line).
    #[test]
    fn gamedata_has_every_bot_event() {
        let events = include_str!("../../../../../gamedata/emevd-bots.json");
        for (table, flag) in TABLES {
            let want = format!("\"flag\": {flag}, \"bots\": {table} }}");
            assert!(events.contains(&want), "missing: {want}");
        }
        let want = format!("\"flag\": {FLAG_CLEAR}, \"bots_clear\": true }}");
        assert!(events.contains(&want), "missing: {want}");
        assert!(events.contains("\"file\": \"event/common.emevd.dcx\""));
    }
}
