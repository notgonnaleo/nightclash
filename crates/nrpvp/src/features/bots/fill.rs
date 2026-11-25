//! BOT-1: fill an expedition with hostile bots (BOTS.md §2a). The OFF-5 director (git `72f7689`, `features/bot_match.rs`), back as a
//! feature. As soon as an expedition is loaded it asks for `want` of the game's own NPC bots and keeps them on the rival team. The game
//! holds an armed table until the host has landed after the hawk crossing, so the bots arrive at the landing (OFF-5 run 2).
//!
//! The game's bot manager holds only 2 bots, so the director works in rounds ("detach and re-arm", MEMORY_MAP §15):
//! 1. set our flag for a bot table (our `common.emevd` event runs `2003[97]`; the manager spawns the table's 2 Nightfarers);
//! 2. wait until the manager's 2 slots hold new handles; keep as many as are still wanted;
//! 3. make the manager forget the kept bots (their slot handle becomes -1); a slot that holds a bot not wanted is left alone and removed
//!    by `2003[98]` at the end;
//! 4. next round with the next table, until the count is reached; then `2003[98]` (removes only what is still in the slots).
//!
//! Every tick each kept bot found in the bot set (`World::bot_chrs`) that is not on the rival team gets it (29: attacks the player,
//! the other bots and monsters; OFF-5 run 2). The roster (alive, HP, position) is what the later cards read (BOT-2 rules, BOT-3
//! brain). No hardware breakpoint, no game call: flags and two kinds of data writes ([`BotWrite`]). Offline mode only in BOT-1.
//! Log lines start with `bots:`.

use super::super::mode::GameMode;
use super::super::{BotWrite, Ctx};
use crate::game::views::Place;
use crate::game::world::World;

/// A round that sees no new bots this long gives up (and the director moves on). Long on purpose: the first round may wait out the
/// rest of the hawk ride (the game spawns an armed table only once the host is on the ground).
pub(crate) const ROUND_TIMEOUT_MS: u64 = 120_000;
/// Pause between forgetting a round's bots and arming the next table.
pub(crate) const ROUND_GAP_MS: u64 = 1_000;
/// The bot tables of the rounds, in order and then again (`NPCBotTableParam`: 10 = heroes 1 + 2, 30 = 3 + 4, 50 = 5 + 6, all level
/// 1), with the flags of their events in `gamedata/emevd-bots.json`.
pub(crate) const ROUND_TABLES: [(i32, u32); 3] =
    [(10, 10_007_300), (30, 10_007_303), (50, 10_007_304)];
/// A director never arms more rounds than this in one expedition (a guard against a manager that never spawns what we keep; mob
/// deaths arm a round each).
pub(crate) const MAX_ROUNDS: usize = 40;
/// BOT-2: a bot killed by a mob is replaced this long after its death (a player's mob death respawns in about 14 s, RW-2).
pub(crate) const RESPAWN_MS: u64 = 14_000;
/// BOT-2: a replacement that has not spawned this long after the death is given up (the bot is then out). The game spawns only while
/// LOCAL stands on the ground, so a LOCAL that is respawning delays it (Leonardo's match of 2026-11-07: a WIN came while the last bot's
/// replacement waited).
pub(crate) const REPLACE_GIVE_UP_MS: u64 = 150_000;
/// The most bots a match asks for (5 rivals for a 6-player Solo match).
pub(crate) const MAX_BOTS: usize = 5;
/// BOT-2 hunt (Leonardo 2026-11-07: bots more aggressive): this often, the living bot farthest from LOCAL, when it is farther than
/// [`HUNT_FAR`], is brought to about 100 m from LOCAL (`BotWrite::Hunt`); one bot at a time, so they come one by one.
pub(crate) const HUNT_EVERY_MS: u64 = 60_000;
/// A bot this far from LOCAL (metres) is a candidate for the hunt.
pub(crate) const HUNT_FAR: f32 = 150.0;
/// Ticks (50 ms) a kept bot that was seen may be missing from the set before it counts as gone (3 s).
const GONE_AFTER: u32 = 60;
/// ARENA-4: the Arena bot's table (`NPCBotTableParam` 30000: the 8 base heroes at soul level 13, OFF-3) and the flag of its event in
/// `gamedata/emevd-bots.json`.
pub(crate) const ARENA_TABLE: (i32, u32) = (30_000, 10_007_305);
/// ARENA-4: where a living bot waits between a round end and the reset, so it cannot hit the frozen or downed LOCAL behind the black
/// screen (Heolstor's arena: flat, centred on (0, 0, 0), at least ±150 m, MEMORY_MAP §27; the spawns are on the x axis).
pub(crate) const ARENA_PARK: [f32; 3] = [0.0, 3.0, 120.0];
/// ARENA-6: where the living bot allies wait between a round end and the reset: on the other side of the arena from the enemy bots'
/// [`ARENA_PARK`], so the two parked groups do not fight (240 m apart).
pub(crate) const ARENA_PARK_ALLY: [f32; 3] = [0.0, 3.0, -120.0];
/// ARENA-4: a bot this close (horizontal metres) to its spawn stands on it.
const ON_SPAWN_M: f32 = 8.0;
/// ARENA-6: the team value of a bot ally: the game's own ally value (a bot spawns on it, `World::rival_bots`; LOCAL is on it in an
/// expedition), so it fights for LOCAL and never hits it (team relation (2, 2) = Friend, MEMORY_MAP §4 / §10).
pub(crate) const ARENA_ALLY_TEAM: u8 = 2;
/// ARENA-6: the team value of the enemy bots in a Duo / Trio match against bots. Not 29: (29, 29) is Rival, so two enemy bots would
/// fight each other. 6 (Enemy) is hostile to 2 both ways (MEMORY_MAP §10: (2, 6) and (6, 2) = Enemy) and a character on 6 cannot hurt
/// another on 6 (MEMORY_MAP §4: a player set to 6 cannot damage enemies). `[HYPOTHESIS: the bot AI on 6 attacks LOCAL and the ally
/// bots and leaves its teammates alone; check in a Duo match against bots]`. A Solo match (one bot) keeps the proven rival value (29).
pub(crate) const ARENA_ENEMY_TEAM: u8 = 6;

/// ARENA-6: a bot's place in an Arena match against bots: on LOCAL's side (`ally`, team A, slots 1.. next to LOCAL's 0) or on the
/// enemy side (team B, slots 0..).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Role {
    pub ally: bool,
    pub slot: u8,
}

/// ARENA-6 (pure): the places of a match of `size` against `size` (1..=3), in the order new bots take them: the enemies first (so Solo
/// is the one enemy of ARENA-4), then the allies.
pub(crate) fn arena_roles(size: u8) -> Vec<Role> {
    let size = size.clamp(1, 3);
    (0..size)
        .map(|slot| Role { ally: false, slot })
        .chain((1..size).map(|slot| Role { ally: true, slot }))
        .collect()
}

/// ARENA-6 (pure): the spawn of a place: the arena's spawn of its team and slot (`game::arenacard`; LOCAL, with no roster team in a
/// match against bots, is team A slot 0).
pub(crate) fn role_spawn(r: Role) -> [f32; 3] {
    crate::game::arenacard::spawn_for(Some(u8::from(!r.ally)), r.slot).0
}

/// ARENA-6 (pure): where a living bot of this place waits between a round end and the reset.
pub(crate) fn role_park(r: Role) -> [f32; 3] {
    if r.ally {
        ARENA_PARK_ALLY
    } else {
        ARENA_PARK
    }
}

/// ARENA-4: what the director needs from the Arena this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArenaIn {
    /// This expedition is an Arena match against bots.
    pub on: bool,
    /// ARENA-6: its team size (1 Solo, 2 Duo, 3 Trio; `mm::arena_bots_size`).
    pub size: u8,
    /// LOCAL stands on its spawn in the arena (the bot is asked for only then).
    pub placed: bool,
    /// The round events so far (`arenarounds::bot_events`).
    pub events: BotEvents,
}

/// ARENA-4: the bot's spawn: B of `game::arenacard` (LOCAL has no roster team in a match against a bot, so it stands on A; a unit test
/// keeps the two equal).
#[cfg(test)]
pub(crate) const ARENA_BOT_SPAWN: [f32; 3] = [15.0, 3.0, 0.0];

/// ARENA-4: a bot the Arena match has, as this feature publishes it every tick (read by `game::arenarounds`). ARENA-6: allies too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArenaBot {
    pub handle: u64,
    /// Its HP when it was first seen (its max-HP field reads 1/3 of it, MEMORY_MAP §18).
    pub full_hp: u32,
    /// Seen on its spawn since the last round end.
    pub ready: bool,
    /// ARENA-6: on LOCAL's side.
    pub ally: bool,
    /// ARENA-6: dead or gone from the bot set (beaten for the rest of the round; replaced at the reset).
    pub out: bool,
}

/// ARENA-4: round events from `game::arenarounds` (counters since the DLL loaded; the director remembers the values it has seen).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BotEvents {
    /// A round ended (a down or the time limit).
    pub round_ends: u32,
    /// The players were reset to their spawns (the next round follows).
    pub resets: u32,
    /// A match was decided (at the last round's reset, before WIN / LOSE).
    pub decided: u32,
}

const EMPTY: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Not in an expedition (or off).
    Idle,
    /// Round `n` armed at `at`.
    Spawning { n: usize, at: u64 },
    /// Round `n` done at `at`; the next round starts after [`ROUND_GAP_MS`].
    Gap { n: usize, at: u64 },
    /// All rounds done (or given up).
    Done,
}

/// BOT-2: who killed a bot (its last attacker at the moment its HP read 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Death {
    /// LOCAL or another rival bot: out of the match for good.
    Rival,
    /// An enemy (any other valid handle): replaced by a new bot after [`RESPAWN_MS`], like a player's mob death (RW-2).
    Mob,
    /// No attacker (the tide, a fall): out for good.
    Env,
}

/// The cause of a bot's death from its last attacker (pure; unit tested).
pub(crate) fn death_cause(attacker: u64, local: Option<u64>, rival_bots: &[u64]) -> Death {
    if attacker == 0 || attacker == u64::MAX {
        Death::Env
    } else if Some(attacker) == local || rival_bots.contains(&attacker) {
        Death::Rival
    } else {
        Death::Mob
    }
}

/// BOT-KF: the killfeed line of a bot's death (pure): killed by LOCAL or another bot, or consumed by the tide. A mob's kill has none
/// (the bot is replaced, like a player's mob death).
pub(crate) fn kill_event(
    handle: u64,
    attacker: u64,
    cause: Death,
    world: &World,
) -> Option<super::super::killfeed::KillEvent> {
    use super::super::killfeed::{short_id, KillEvent, KillKind};
    use crate::game::world::bot_session_id;
    let victim = short_id(bot_session_id(handle));
    match cause {
        Death::Mob => None,
        Death::Env => Some(KillEvent {
            kind: KillKind::Tide,
            victim,
            killer: None,
        }),
        Death::Rival => {
            let killer = if Some(attacker) == world.local_handle {
                world
                    .fighters
                    .iter()
                    .flatten()
                    .find(|f| f.local)
                    .map(|f| short_id(f.id))
            } else {
                Some(short_id(bot_session_id(attacker)))
            };
            Some(KillEvent {
                kind: KillKind::Killed,
                victim,
                killer,
            })
        }
    }
}

/// One kept bot and what the roster knows about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Kept {
    pub handle: u64,
    /// Found in the set at least once (logged once).
    pub seen: bool,
    /// The team last written (0 = none yet), so each change is logged once.
    applied: u8,
    /// Ticks in a row it was not found in the set.
    misses: u32,
    /// Seen before and missing for [`GONE_AFTER`] ticks: removed from the world.
    pub gone: bool,
    /// HP read 0 (logged once; whether a dead bot leaves the set is evidence for BOT-2).
    pub dead: bool,
    pub hp: u32,
    pub max_hp: u32,
    pub pos: Option<[f32; 3]>,
    /// Sent to a camp once (`BotWrite::Spread`).
    spread: bool,
    /// BOT-2: how it died and when (ms), once its HP read 0.
    pub death: Option<(Death, u64)>,
    /// BOT-2: killed by a mob and its replacement was asked for: no longer one of the wanted bots.
    pub replaced: bool,
    /// ARENA-4: its HP when first seen above 0 (its full HP: the max-HP field reads 1/3, MEMORY_MAP §18).
    pub full_hp: u32,
    /// ARENA-6: its place in an Arena match against bots (`None` outside the Arena).
    pub role: Option<Role>,
    /// ARENA-4: it has stood on its spawn since the last round end.
    pub arena_ready: bool,
}

impl Kept {
    fn new(handle: u64) -> Self {
        Kept {
            handle,
            seen: false,
            applied: 0,
            misses: 0,
            gone: false,
            dead: false,
            hp: 0,
            max_hp: 0,
            pos: None,
            spread: false,
            death: None,
            replaced: false,
            full_hp: 0,
            role: None,
            arena_ready: false,
        }
    }
}

pub(crate) struct BotFill {
    /// Bots wanted (`bots_fill`, 0..=5; 0 = off).
    want: usize,
    /// Team value written on kept bots (`pvpve_team`).
    rival_team: u8,
    pub(crate) stage: Stage,
    pub(crate) kept: Vec<Kept>,
    /// Handles seen in the manager's slots and dealt with (kept or left for the clear), so they are not counted twice.
    handled: Vec<u64>,
    /// Bots spawned but not needed (the clear removes them about a second later); each is still sent away from LOCAL once, so no bot
    /// is ever seen at LOCAL's side. `true` = sent.
    extra: Vec<(u64, bool)>,
    /// Rounds armed in this expedition.
    rounds: usize,
    /// Send each kept bot to a far camp once (`bots_spread`, on by default).
    pub(crate) spread: bool,
    /// LOCAL's world position at the last step (the roster's distances).
    me: Option<[f32; 3]>,
    /// BOT-5: this expedition is a match against bots (Bots On at Commence): active in any mode, with [`MAX_BOTS`] bots.
    bots_match: bool,
    /// The count from the config / console, put back when a bots match ends.
    cfg_want: usize,
    /// An expedition this director was active in has just ended (read once by [`BotFill::take_over`]).
    over: bool,
    /// BOT-2 hunt: when the last hunt was asked for (ms; the first comes [`HUNT_EVERY_MS`] after the fill is done).
    last_hunt: Option<u64>,
    /// BOT-2: since when a replacement has been waiting (ms), for [`REPLACE_GIVE_UP_MS`].
    waiting_since: Option<u64>,
    /// Bring far bots to LOCAL (`bots_hunt`, on by default).
    pub(crate) hunt: bool,
    /// ARENA-4: the Arena this tick ([`BotFill::set_arena`]).
    arena: ArenaIn,
    /// ARENA-4: the round events already handled (taken at the first active tick of the match).
    arena_seen: Option<BotEvents>,
    /// ARENA-4: the match is decided: no more bots.
    arena_done: bool,
}

impl BotFill {
    pub(crate) fn new(want: usize, rival_team: u8) -> Self {
        BotFill {
            want: want.min(MAX_BOTS),
            rival_team,
            stage: Stage::Idle,
            kept: Vec::new(),
            handled: Vec::new(),
            extra: Vec::new(),
            rounds: 0,
            spread: true,
            me: None,
            bots_match: false,
            cfg_want: want.min(MAX_BOTS),
            over: false,
            last_hunt: None,
            waiting_since: None,
            hunt: true,
            arena: ArenaIn::default(),
            arena_seen: None,
            arena_done: false,
        }
    }

    /// ARENA-4: the Arena this tick (before [`BotFill::step`]).
    pub(crate) fn set_arena(&mut self, a: ArenaIn) {
        self.arena = a;
    }

    /// ARENA-4 / ARENA-6: the Arena's bots that play this round (kept, not replaced, seen once), for `arenarounds`.
    pub(crate) fn arena_bots(&self) -> Vec<ArenaBot> {
        if !self.arena.on {
            return Vec::new();
        }
        self.kept
            .iter()
            .filter(|k| !k.replaced && k.seen)
            .filter_map(|k| {
                k.role.map(|r| ArenaBot {
                    handle: k.handle,
                    full_hp: k.full_hp,
                    ready: k.arena_ready,
                    ally: r.ally,
                    out: k.gone || k.dead,
                })
            })
            .collect()
    }

    /// ARENA-4: the first enemy bot still in (the Solo match's one bot; tests).
    #[cfg(test)]
    pub(crate) fn arena_bot(&self) -> Option<ArenaBot> {
        self.arena_bots().into_iter().find(|b| !b.ally && !b.out)
    }

    /// ARENA-6: every place of the match is held by a bot that has stood on its spawn since the last round end (a round start waits for
    /// it).
    pub(crate) fn arena_all_ready(&self) -> bool {
        self.arena.on
            && arena_roles(self.arena.size).iter().all(|r| {
                self.kept
                    .iter()
                    .any(|k| !k.replaced && k.role == Some(*r) && k.arena_ready)
            })
    }

    /// ARENA-6: the first place no kept bot holds (a new bot takes it).
    fn free_role(&self) -> Option<Role> {
        arena_roles(self.arena.size)
            .into_iter()
            .find(|r| !self.kept.iter().any(|k| !k.replaced && k.role == Some(*r)))
    }

    pub(crate) fn want(&self) -> usize {
        self.want
    }

    /// Kept bots that still play for the fill count (not replaced after a mob death).
    fn active(&self) -> usize {
        self.kept.iter().filter(|k| !k.replaced).count()
    }

    /// BOT-2: bots killed by a mob whose replacement has not been kept yet (they still count as rivals for the match decision).
    pub(crate) fn pending(&self) -> usize {
        let dying = self
            .kept
            .iter()
            .filter(|k| matches!(k.death, Some((Death::Mob, _))) && !k.replaced)
            .count();
        let unfilled = if self.kept.iter().any(|k| k.replaced) {
            self.want.saturating_sub(self.active())
        } else {
            0
        };
        dying + unfilled
    }

    /// BOT-5: whether the current (or next) expedition is a match against bots.
    pub(crate) fn set_bots_match(&mut self, on: bool) {
        self.bots_match = on;
    }

    /// BOT-5: true once after an expedition this director was active in ended.
    pub(crate) fn take_over(&mut self) -> bool {
        std::mem::take(&mut self.over)
    }

    /// Change the wanted count (console `bots_fill`). In an expedition that is done, a higher count arms more rounds.
    pub(crate) fn set_want(&mut self, n: usize, now: u64) {
        self.want = n.min(MAX_BOTS);
        self.cfg_want = self.want;
        if self.stage == Stage::Done && self.active() < self.want && self.rounds < MAX_ROUNDS {
            self.stage = Stage::Gap {
                n: self.rounds - 1,
                at: now.saturating_sub(ROUND_GAP_MS),
            };
        }
        tracing::info!("bots: fill count is now {}", self.want);
    }

    /// One step (pure apart from logging; unit tested with a made-up clock).
    pub(crate) fn step(&mut self, now: u64, mode: GameMode, world: &World, ctx: &mut Ctx) {
        let arena = self.arena.on;
        // ARENA-4: the Arena's bot is asked for only once LOCAL stands on its spawn (after the warp's map load).
        let active = (mode == GameMode::Offline || self.bots_match || (arena && self.arena.placed))
            && world.in_expedition();
        let size = self.arena.size.clamp(1, 3);
        let arena_want = usize::from(2 * size - 1);
        if active && arena && self.want != arena_want && self.stage == Stage::Idle {
            tracing::info!(
                "bots: Arena match against bots: {size} vs {size}, {arena_want} bot(s) (table {})",
                ARENA_TABLE.0
            );
            self.want = arena_want;
        } else if active
            && !arena
            && self.bots_match
            && self.want < MAX_BOTS
            && self.stage == Stage::Idle
        {
            tracing::info!("bots: match against bots: {MAX_BOTS} bots");
            self.want = MAX_BOTS;
        }
        if !active {
            let over = world.wcm_instance == 0 || world.place == Some(Place::Roundtable);
            if self.stage != Stage::Idle && over {
                // Load screen, title or back in the Roundtable: the expedition is over; its bots are gone with it.
                tracing::info!(
                    "bots: expedition over; {} bot(s) were kept",
                    self.kept.len()
                );
                self.reset();
            }
            return;
        }
        self.me = world.local_pos.map(|(p, _)| p);
        self.keep_rivals(now, world, ctx);
        if arena {
            self.arena_step(world, ctx);
        } else {
            self.respawn_due(now);
            self.give_up_replacements(now);
        }
        if self.want == 0 && self.stage == Stage::Idle {
            return;
        }
        let slots = world
            .bots
            .map(|b| [b.heads[0], b.heads[1]])
            .unwrap_or([EMPTY; 2]);
        if world.bots.is_some() {
            self.release_extras(slots);
        }
        match self.stage {
            Stage::Idle => {
                tracing::info!(
                    "bots: expedition: filling with {} bot(s); the game brings them at the landing",
                    self.want
                );
                self.arm(0, now, ctx);
            }
            Stage::Spawning { n, at } => {
                let fresh: Vec<(usize, u64)> = slots
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| **h != EMPTY && **h != 0 && !self.handled.contains(h))
                    .map(|(i, h)| (i, *h))
                    .collect();
                let both = slots.iter().all(|h| *h != EMPTY && *h != 0);
                if both && !fresh.is_empty() {
                    for (slot, h) in fresh {
                        self.handled.push(h);
                        if self.active() < self.want {
                            let role = if arena { self.free_role() } else { None };
                            self.kept.push(Kept {
                                role,
                                ..Kept::new(h)
                            });
                            ctx.bot_writes.push(BotWrite::ForgetSlot(slot));
                            tracing::info!(
                                "bots: round {}: kept bot 0x{h:X} (slot {slot}); {} of {}{}",
                                n + 1,
                                self.active(),
                                self.want,
                                role.map_or(String::new(), |r| format!(
                                    "; Arena {} slot {}",
                                    if r.ally { "ally" } else { "enemy" },
                                    r.slot
                                ))
                            );
                        } else {
                            self.extra.push((h, false));
                            tracing::info!(
                                "bots: round {}: bot 0x{h:X} (slot {slot}) not needed; sent away, the clear removes it",
                                n + 1
                            );
                        }
                    }
                    self.stage = Stage::Gap { n, at: now };
                } else if now.saturating_sub(at) >= ROUND_TIMEOUT_MS {
                    tracing::warn!(
                        "bots: round {}: no new bots after {} s (slots {:X?}); moving on",
                        n + 1,
                        ROUND_TIMEOUT_MS / 1000,
                        slots
                    );
                    self.stage = Stage::Gap { n, at: now };
                }
            }
            Stage::Gap { n, at } => {
                if now.saturating_sub(at) < ROUND_GAP_MS {
                    return;
                }
                if self.active() < self.want && self.rounds < MAX_ROUNDS {
                    self.arm(n + 1, now, ctx);
                } else {
                    ctx.flags.push((super::FLAG_CLEAR, true));
                    tracing::info!(
                        "bots: fill done: {} of {} bot(s) kept; clearing the manager",
                        self.active(),
                        self.want
                    );
                    self.stage = Stage::Done;
                }
            }
            Stage::Done if arena => {
                // ARENA-4: a bot replaced at a reset: one more round of the table.
                if self.active() < self.want && !self.arena_done && self.rounds < MAX_ROUNDS {
                    self.stage = Stage::Gap {
                        n: self.rounds.saturating_sub(1),
                        at: now.saturating_sub(ROUND_GAP_MS),
                    };
                }
            }
            Stage::Done => self.hunt_step(now, world, ctx),
        }
    }

    /// ARENA-4 / ARENA-6: the Arena's bots follow the rounds (`arenarounds` events): parked at a round end (allies and enemies on
    /// opposite sides); at the reset healed and put back on their spawns, or replaced when dead; put at 0 HP when the match is decided.
    /// A new bot is put on the spawn of its place once. A replaced bot that comes back to life is put at 0 HP again.
    fn arena_step(&mut self, world: &World, ctx: &mut Ctx) {
        let ev = self.arena.events;
        let seen = *self.arena_seen.get_or_insert(ev);
        self.arena_seen = Some(ev);
        let round_end = ev.round_ends != seen.round_ends;
        let reset = ev.resets != seen.resets && !self.arena_done;
        let decided = ev.decided != seen.decided;
        let live = |k: &Kept| {
            world
                .bot(k.handle)
                .filter(|b| b.hp > 0 && k.seen && !k.gone)
                .copied()
        };
        let mut lost = 0;
        for k in self.kept.iter_mut().filter(|k| !k.replaced) {
            let Some(role) = k.role else {
                continue;
            };
            let b = live(k);
            if round_end {
                k.arena_ready = false;
                if let Some(b) = b {
                    tracing::info!(
                        "bots: Arena round over: bot 0x{:X} parked until the reset",
                        b.handle
                    );
                    ctx.bot_writes.push(BotWrite::Put {
                        chr: b.chr,
                        handle: b.handle,
                        to: role_park(role),
                    });
                }
            }
            if reset {
                if let Some(b) = b {
                    let full = k.full_hp.max(b.hp);
                    tracing::info!(
                        "bots: Arena reset: bot 0x{:X} back to its spawn, HP {} -> {full}",
                        b.handle,
                        b.hp
                    );
                    ctx.bot_writes.push(BotWrite::Hp {
                        chr: b.chr,
                        handle: b.handle,
                        hp: full,
                    });
                    ctx.bot_writes.push(BotWrite::Put {
                        chr: b.chr,
                        handle: b.handle,
                        to: role_spawn(role),
                    });
                } else {
                    k.replaced = true;
                    lost += 1;
                }
            }
            if decided {
                k.arena_ready = false;
                if let Some(b) = b {
                    tracing::info!(
                        "bots: Arena match decided: bot 0x{:X} removed (HP 0)",
                        b.handle
                    );
                    ctx.bot_writes.push(BotWrite::Hp {
                        chr: b.chr,
                        handle: b.handle,
                        hp: 0,
                    });
                }
            }
        }
        if reset && lost > 0 {
            tracing::info!("bots: Arena reset: {lost} bot(s) dead; new ones for the next round");
        }
        if decided {
            self.arena_done = true;
        }
        for k in &mut self.kept {
            let Some(b) = world.bot(k.handle) else {
                continue;
            };
            if !k.replaced && !k.spread && b.pos.is_some() && !self.arena_done {
                if let Some(role) = k.role {
                    // A new bot: on its spawn once (the game brings it next to LOCAL).
                    k.spread = true;
                    ctx.bot_writes.push(BotWrite::Put {
                        chr: b.chr,
                        handle: k.handle,
                        to: role_spawn(role),
                    });
                }
            } else if k.replaced && b.hp > 0 && k.seen {
                tracing::info!("bots: Arena: old bot 0x{:X} is alive again; HP 0", k.handle);
                ctx.bot_writes.push(BotWrite::Hp {
                    chr: b.chr,
                    handle: k.handle,
                    hp: 0,
                });
            }
        }
        // Standing on its spawn (seen after the put) makes the bot ready for the next round start (not on the tick a round ended: the
        // park is not written yet).
        if !round_end && !self.arena_done {
            for k in self.kept.iter_mut().filter(|k| !k.replaced) {
                let Some(role) = k.role else {
                    continue;
                };
                let spawn = role_spawn(role);
                let on_spawn = live(k).and_then(|b| b.pos).is_some_and(|p| {
                    ((p[0] - spawn[0]).powi(2) + (p[2] - spawn[2]).powi(2)).sqrt() <= ON_SPAWN_M
                });
                if on_spawn && k.spread && !k.arena_ready {
                    k.arena_ready = true;
                    tracing::info!(
                        "bots: Arena bot 0x{:X} stands on its spawn ({} slot {})",
                        k.handle,
                        if role.ally { "ally" } else { "enemy" },
                        role.slot
                    );
                }
            }
        }
    }

    /// BOT-2 hunt: every [`HUNT_EVERY_MS`] the farthest living bot beyond [`HUNT_FAR`] is brought near LOCAL (LOCAL alive and on the ground).
    fn hunt_step(&mut self, now: u64, world: &World, ctx: &mut Ctx) {
        let last = *self.last_hunt.get_or_insert(now);
        if !self.hunt || now.saturating_sub(last) < HUNT_EVERY_MS {
            return;
        }
        let Some((me, true)) = world.local_pos else {
            return;
        };
        if world.local_state.is_some_and(|(_, s)| s != 0)
            || world
                .fighters
                .iter()
                .flatten()
                .any(|f| f.local && f.hp == 0)
        {
            return;
        }
        self.last_hunt = Some(now);
        let far = self
            .kept
            .iter()
            .filter(|k| k.seen && !k.gone && !k.dead && !k.replaced)
            .filter_map(|k| {
                let b = world.bot(k.handle)?;
                let p = b.pos?;
                let d = ((p[0] - me[0]).powi(2) + (p[2] - me[2]).powi(2)).sqrt();
                (d > HUNT_FAR).then_some((d, b.chr, k.handle))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, chr, handle)) = far {
            tracing::info!("bots: hunt: bot 0x{handle:X} is {d:.0} m away; bringing it near LOCAL");
            ctx.bot_writes.push(BotWrite::Hunt { chr, handle });
        }
    }

    /// A bot not needed leaves its slot with the clear (`2003[98]`), and the game may give its handle to a later new bot: Leonardo's
    /// Trio match against bots (2026-11-25, testing-log `2026-11-25-arena-trio-bots`) had 0x...0B, not needed in the first fill, back
    /// in slot 0 at the round 2 refill; it was skipped as already handled and the refill brought 1 of 3 enemies. So once a bot not
    /// needed is out of both slots it is forgotten, and the same handle in a slot later counts as a new bot.
    fn release_extras(&mut self, slots: [u64; 2]) {
        let handled = &mut self.handled;
        self.extra.retain(|(h, _)| {
            let still = slots.contains(h);
            if !still {
                handled.retain(|x| x != h);
            }
            still
        });
    }

    fn arm(&mut self, n: usize, now: u64, ctx: &mut Ctx) {
        let (table, flag) = if self.arena.on {
            ARENA_TABLE
        } else {
            ROUND_TABLES[n % ROUND_TABLES.len()]
        };
        ctx.flags.push((flag, true));
        self.rounds += 1;
        tracing::info!("bots: round {}: table {table} (flag {flag})", n + 1);
        self.stage = Stage::Spawning { n, at: now };
    }

    /// Keep every found bot on the rival team and refresh the roster.
    fn keep_rivals(&mut self, now: u64, world: &World, ctx: &mut Ctx) {
        let ground = world.local_pos.is_some_and(|(_, g)| g);
        // ARENA-4: no camps to send bots to in the arena (the extra is cleared within a second, behind the black screen).
        let arena = self.arena.on;
        for (h, sent) in &mut self.extra {
            if *sent || !self.spread || !ground || arena {
                continue;
            }
            if let Some(b) = world.bot(*h).filter(|b| b.pos.is_some()) {
                *sent = true;
                ctx.bot_writes.push(BotWrite::Spread {
                    chr: b.chr,
                    handle: *h,
                    sector: 0,
                    sectors: 1,
                });
            }
        }
        let rival_team = self.rival_team;
        // ARENA-6: in a Duo / Trio match against bots each bot's team follows its place; Solo keeps the rival value.
        let duo_trio = arena && self.arena.size > 1;
        let rivals: Vec<u64> = world.rival_bots().map(|b| b.handle).collect();
        let sectors = self.want.max(self.kept.len());
        let spread_on = self.spread && !arena;
        for (i, k) in self.kept.iter_mut().enumerate() {
            if k.gone {
                continue;
            }
            let Some(b) = world.bot(k.handle) else {
                k.misses += 1;
                if k.seen && k.misses >= GONE_AFTER {
                    k.gone = true;
                    tracing::info!(
                        "bots: bot 0x{:X} left the bot set (dead: {})",
                        k.handle,
                        k.dead
                    );
                }
                continue;
            };
            k.misses = 0;
            k.hp = b.hp;
            k.max_hp = b.max_hp;
            k.pos = b.pos.or(k.pos);
            if k.full_hp == 0 && b.hp > 0 {
                k.full_hp = b.hp;
            }
            if !k.seen {
                k.seen = true;
                if b.npc_id != crate::game::layout::BOT_NPC_PARAM {
                    tracing::warn!(
                        "bots: bot 0x{:X} has npc id {}, not the bot tables' {}",
                        k.handle,
                        b.npc_id,
                        crate::game::layout::BOT_NPC_PARAM
                    );
                }
                tracing::info!(
                    "bots: bot 0x{:X} is ChrIns 0x{:X} (npc id {}, team {}, hp {}/{})",
                    k.handle,
                    b.chr,
                    b.npc_id,
                    b.team,
                    b.hp,
                    b.max_hp
                );
            }
            if b.hp == 0 && b.max_hp > 0 && !k.dead {
                k.dead = true;
                let cause = death_cause(b.attacker, world.local_handle, &rivals);
                k.death = Some((cause, now));
                // ARENA-4: the Arena's round messages say who won; no killfeed line there.
                if let Some(ev) = kill_event(k.handle, b.attacker, cause, world).filter(|_| !arena)
                {
                    ctx.kills.push(ev);
                }
                let next = if arena {
                    "; the Arena round is over".to_string()
                } else if cause == Death::Mob {
                    format!("; a new bot replaces it in {} s", RESPAWN_MS / 1000)
                } else {
                    "; out of the match".to_string()
                };
                tracing::info!(
                    "bots: bot 0x{:X} is dead (hp 0; last attacker 0x{:X} = {cause:?}){next}",
                    k.handle,
                    b.attacker
                );
            } else if b.hp > 0 && k.dead {
                k.dead = false;
                tracing::info!("bots: bot 0x{:X} is alive again (hp {})", k.handle, b.hp);
            }
            if spread_on
                && !k.spread
                && b.pos.is_some()
                && world.local_pos.is_some_and(|(_, ground)| ground)
            {
                k.spread = true;
                ctx.bot_writes.push(BotWrite::Spread {
                    chr: b.chr,
                    handle: k.handle,
                    sector: i,
                    sectors,
                });
            }
            let target = match k.role {
                Some(r) if duo_trio && r.ally => ARENA_ALLY_TEAM,
                Some(_) if duo_trio => ARENA_ENEMY_TEAM,
                _ => rival_team,
            };
            if b.team != target {
                ctx.bot_writes.push(BotWrite::Team {
                    chr: b.chr,
                    handle: k.handle,
                    team: target,
                });
                if k.applied != target {
                    k.applied = target;
                    tracing::info!("bots: bot 0x{:X}: team {} -> {}", k.handle, b.team, target);
                }
            }
        }
    }

    /// BOT-2: a bot killed by a mob [`RESPAWN_MS`] ago stops counting; a done fill arms one more round for its replacement.
    fn respawn_due(&mut self, now: u64) {
        let mut due = 0;
        for k in &mut self.kept {
            if let Some((Death::Mob, at)) = k.death {
                if !k.replaced && now.saturating_sub(at) >= RESPAWN_MS {
                    k.replaced = true;
                    due += 1;
                }
            }
        }
        if due > 0 && self.stage == Stage::Done && self.rounds < MAX_ROUNDS {
            tracing::info!("bots: {due} bot(s) killed by mobs: arming a replacement round");
            self.stage = Stage::Gap {
                n: self.rounds - 1,
                at: now.saturating_sub(ROUND_GAP_MS),
            };
        }
    }

    /// BOT-2: a replacement still missing after [`REPLACE_GIVE_UP_MS`] is given up: the fill count drops to the bots it has, so the match
    /// can be decided.
    fn give_up_replacements(&mut self, now: u64) {
        if self.pending() == 0 {
            self.waiting_since = None;
            return;
        }
        let since = *self.waiting_since.get_or_insert(now);
        if now.saturating_sub(since) >= REPLACE_GIVE_UP_MS {
            let have = self.active();
            tracing::warn!(
                "bots: a replacement did not spawn in {} s; going on with {have} bot(s)",
                REPLACE_GIVE_UP_MS / 1000
            );
            for k in &mut self.kept {
                if matches!(k.death, Some((Death::Mob, _))) {
                    k.replaced = true;
                }
            }
            self.want = self.active();
            self.waiting_since = None;
        }
    }

    fn reset(&mut self) {
        self.arena_seen = None;
        self.arena_done = false;
        self.over = true;
        self.waiting_since = None;
        self.want = self.cfg_want;
        self.stage = Stage::Idle;
        self.kept.clear();
        self.handled.clear();
        self.extra.clear();
        self.last_hunt = None;
        self.rounds = 0;
    }

    /// Bots kept and still in the world with HP above 0.
    pub(crate) fn alive(&self) -> usize {
        self.kept
            .iter()
            .filter(|k| k.seen && !k.gone && !k.dead && !k.replaced)
            .count()
    }

    /// The roster for the console: a summary line, then one line per kept bot.
    pub(crate) fn roster(&self) -> String {
        let mut out = format!(
            "bots: {:?}, {} of {} kept, {} alive",
            self.stage,
            self.active(),
            self.want,
            self.alive()
        );
        if self.arena.on {
            out.push_str(&format!(
                ", Arena {0} vs {0}: {1}",
                self.arena.size,
                match (self.arena_done, self.arena_all_ready()) {
                    (true, _) => "done (match decided)",
                    (false, true) => "every bot on its spawn",
                    (false, false) => "not every bot on its spawn",
                }
            ));
        }
        for (i, k) in self.kept.iter().enumerate() {
            let state = match (k.gone, k.seen, k.death) {
                _ if k.replaced => "dead (mob), replaced",
                (true, _, _) => "gone",
                (_, false, _) => "not found yet",
                (_, _, Some((Death::Mob, _))) => "dead (mob), respawning",
                (_, _, Some((Death::Rival, _))) => "dead (rival), out",
                (_, _, Some((Death::Env, _))) => "dead (tide or fall), out",
                _ if k.dead => "dead",
                _ => "alive",
            };
            let dist = match (k.pos, self.me) {
                (Some(p), Some(m)) => format!(
                    ", {:.0} m away",
                    ((p[0] - m[0]).powi(2) + (p[2] - m[2]).powi(2)).sqrt()
                ),
                _ => String::new(),
            };
            let pos = k
                .pos
                .map(|p| format!(" at ({:.0}, {:.0}, {:.0}){dist}", p[0], p[1], p[2]))
                .unwrap_or_default();
            let team = if k.applied == 0 {
                "-".to_string()
            } else {
                k.applied.to_string()
            };
            out.push_str(&format!(
                "\n  b{} 0x{:X}: {state}, hp {}/{}, team {team}{pos}",
                i + 1,
                k.handle,
                k.hp,
                k.max_hp
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::views::BotManState;
    use crate::game::world::BotChr;

    fn world(slots: [u64; 2]) -> World {
        let mut heads = [0u64; 8];
        heads[..2].copy_from_slice(&slots);
        World {
            wcm_instance: 0x5_0000,
            local: Some(0x10_0000),
            place: Some(Place::Expedition),
            bots: Some(BotManState {
                table: 10,
                armed: false,
                bots: 2,
                heads,
            }),
            ..World::default()
        }
    }

    const E: u64 = EMPTY;

    /// Drive the director through the rounds; the manager fills both slots with new handles after each arm.
    #[test]
    fn five_bots_in_three_rounds() {
        let mut d = BotFill::new(5, 29);
        let mut t = 0;
        let mut go = |d: &mut BotFill, w: &World, ctx: &mut Ctx, dt: u64| {
            t += dt;
            d.step(t, GameMode::Offline, w, ctx);
        };
        // The expedition is loaded: round 1 is armed at once (the game waits for the landing itself).
        let mut ctx = Ctx::default();
        go(&mut d, &world([E, E]), &mut ctx, 0);
        assert_eq!(ctx.flags, vec![(10_007_300, true)]);
        // Nothing yet during the ride.
        let mut ctx = Ctx::default();
        go(&mut d, &world([E, E]), &mut ctx, 50_000);
        assert_eq!(ctx, Ctx::default());
        // Round 1 spawns 2: both kept and forgotten.
        let mut ctx = Ctx::default();
        go(&mut d, &world([0xA0, 0xA1]), &mut ctx, 50);
        assert_eq!(
            ctx.bot_writes,
            vec![BotWrite::ForgetSlot(0), BotWrite::ForgetSlot(1)]
        );
        // Gap, then round 2.
        let mut ctx = Ctx::default();
        go(&mut d, &world([E, E]), &mut ctx, ROUND_GAP_MS);
        assert_eq!(ctx.flags, vec![(10_007_303, true)]);
        go(&mut d, &world([0xB0, 0xB1]), &mut ctx, 50);
        go(&mut d, &world([E, E]), &mut ctx, ROUND_GAP_MS);
        // Round 3: only one more is wanted; slot 1 stays for the clear.
        let mut ctx = Ctx::default();
        go(&mut d, &world([0xC0, 0xC1]), &mut ctx, 50);
        assert_eq!(ctx.bot_writes, vec![BotWrite::ForgetSlot(0)]);
        let mut ctx = Ctx::default();
        go(&mut d, &world([E, 0xC1]), &mut ctx, ROUND_GAP_MS);
        assert_eq!(ctx.flags, vec![(super::super::FLAG_CLEAR, true)]);
        assert_eq!(d.kept.len(), 5);
        assert_eq!(d.stage, Stage::Done);
        // The same count again changes nothing.
        d.set_want(5, t);
        assert_eq!(d.stage, Stage::Done);
    }

    #[test]
    fn found_bots_are_made_rivals_and_the_roster_follows_them() {
        let mut d = BotFill::new(1, 29);
        d.stage = Stage::Done;
        d.kept.push(Kept::new(0xA0));
        let mut w = world([E, E]);
        w.bot_chrs.push(BotChr {
            handle: 0xA0,
            chr: 0x7000,
            team: 2,
            npc_id: 600_040_000,
            hp: 700,
            max_hp: 720,
            pos: Some([1.0, 2.0, 3.0]),
            attacker: 0,
        });
        let mut ctx = Ctx::default();
        d.step(0, GameMode::Offline, &w, &mut ctx);
        assert_eq!(
            ctx.bot_writes,
            vec![BotWrite::Team {
                chr: 0x7000,
                handle: 0xA0,
                team: 29
            }]
        );
        assert_eq!(d.alive(), 1);
        let r = d.roster();
        assert!(
            r.contains("b1 0xA0: alive, hp 700/720, team 29 at (1, 2, 3)"),
            "{r}"
        );
        // Already a rival: nothing to write. HP 0: dead.
        w.bot_chrs[0].team = 29;
        w.bot_chrs[0].hp = 0;
        let mut ctx = Ctx::default();
        d.step(50, GameMode::Offline, &w, &mut ctx);
        assert!(ctx.bot_writes.is_empty());
        assert_eq!(d.alive(), 0);
        assert!(d.roster().contains("dead"));
        // Missing from the set for 3 s: gone.
        w.bot_chrs.clear();
        for i in 0..u64::from(GONE_AFTER) {
            d.step(100 + i * 50, GameMode::Offline, &w, &mut ctx);
        }
        assert!(d.kept[0].gone);
    }

    #[test]
    fn a_higher_count_after_the_fill_arms_another_round() {
        let mut d = BotFill::new(1, 29);
        let mut ctx = Ctx::default();
        d.step(0, GameMode::Offline, &world([E, E]), &mut ctx);
        d.step(50, GameMode::Offline, &world([0xA0, 0xA1]), &mut ctx);
        d.step(
            50 + ROUND_GAP_MS,
            GameMode::Offline,
            &world([E, 0xA1]),
            &mut ctx,
        );
        assert_eq!(d.stage, Stage::Done);
        d.set_want(3, 2_000);
        let mut ctx = Ctx::default();
        d.step(2_000, GameMode::Offline, &world([E, E]), &mut ctx);
        assert_eq!(ctx.flags, vec![(10_007_303, true)], "round 2 = table 30");
    }

    #[test]
    fn the_cause_of_a_bots_death_comes_from_its_last_attacker() {
        assert_eq!(death_cause(0, Some(0xF0), &[]), Death::Env);
        assert_eq!(death_cause(u64::MAX, None, &[]), Death::Env);
        assert_eq!(death_cause(0xF0, Some(0xF0), &[]), Death::Rival);
        assert_eq!(death_cause(0xA1, Some(0xF0), &[0xA0, 0xA1]), Death::Rival);
        assert_eq!(
            death_cause(0x3200_0000_1100_0004, Some(0xF0), &[0xA0]),
            Death::Mob
        );
    }

    #[test]
    fn a_bot_killed_by_a_mob_is_replaced_after_the_respawn_delay() {
        let mut d = BotFill::new(1, 29);
        let mut ctx = Ctx::default();
        d.step(0, GameMode::Offline, &world([E, E]), &mut ctx);
        d.step(50, GameMode::Offline, &world([0xA0, 0xA1]), &mut ctx);
        d.step(
            50 + ROUND_GAP_MS,
            GameMode::Offline,
            &world([E, 0xA1]),
            &mut ctx,
        );
        assert_eq!(d.stage, Stage::Done);
        let mut w = world([E, E]);
        let bot = |hp, attacker| BotChr {
            handle: 0xA0,
            chr: 0x7000,
            team: 29,
            npc_id: 600_040_000,
            hp,
            max_hp: 240,
            pos: None,
            attacker,
        };
        w.bot_chrs = vec![bot(700, 0)];
        d.step(2_000, GameMode::Offline, &w, &mut ctx);
        // A camp mob kills it.
        w.bot_chrs = vec![bot(0, 0x3200_0000_1100_0004)];
        d.step(3_000, GameMode::Offline, &w, &mut ctx);
        assert!(d.roster().contains("respawning"), "{}", d.roster());
        let mut ctx = Ctx::default();
        d.step(3_000 + RESPAWN_MS - 1, GameMode::Offline, &w, &mut ctx);
        assert!(ctx.flags.is_empty(), "not yet");
        d.step(3_000 + RESPAWN_MS, GameMode::Offline, &w, &mut ctx);
        d.step(3_050 + RESPAWN_MS, GameMode::Offline, &w, &mut ctx);
        assert_eq!(ctx.flags, vec![(10_007_303, true)], "a replacement round");
        assert_eq!(d.active(), 0);
        assert_eq!(d.pending(), 1, "still owed a bot");
        // The replacement never comes: given up after REPLACE_GIVE_UP_MS.
        d.step(3_000 + REPLACE_GIVE_UP_MS, GameMode::Offline, &w, &mut ctx);
        d.step(
            3_050 + REPLACE_GIVE_UP_MS * 2,
            GameMode::Offline,
            &w,
            &mut ctx,
        );
        assert_eq!(d.pending(), 0);
        assert!(ctx.kills.is_empty(), "a mob's kill has no killfeed line");
        // Killed by LOCAL instead: out, no replacement.
        let mut d = BotFill::new(1, 29);
        d.stage = Stage::Done;
        d.rounds = 1;
        d.kept.push(Kept::new(0xA0));
        w.local_handle = Some(0xF0);
        w.bot_chrs = vec![bot(0, 0xF0)];
        let mut ctx = Ctx::default();
        d.step(0, GameMode::Offline, &w, &mut ctx);
        d.step(RESPAWN_MS * 2, GameMode::Offline, &w, &mut ctx);
        assert!(ctx.flags.is_empty());
        assert!(d.roster().contains("dead (rival), out"), "{}", d.roster());
        // The killfeed: LOCAL's handle names LOCAL's session id (no LOCAL fighter in this world: no killer name).
        assert_eq!(
            ctx.kills,
            vec![super::super::super::killfeed::KillEvent {
                kind: super::super::super::killfeed::KillKind::Killed,
                victim: 0xB0A0,
                killer: None,
            }]
        );
    }

    #[test]
    fn nothing_happens_outside_offline_or_when_off() {
        for (want, mode) in [(5, GameMode::BattleRoyale), (0, GameMode::Offline)] {
            let mut d = BotFill::new(want, 29);
            let mut ctx = Ctx::default();
            d.step(0, mode, &world([E, E]), &mut ctx);
            d.step(ROUND_TIMEOUT_MS, mode, &world([E, E]), &mut ctx);
            assert_eq!(ctx, Ctx::default());
            assert_eq!(d.stage, Stage::Idle);
        }
    }

    #[test]
    fn a_bots_match_fills_five_in_any_mode_and_ends_with_the_expedition() {
        let mut d = BotFill::new(0, 29);
        d.set_bots_match(true);
        let mut ctx = Ctx::default();
        d.step(0, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        assert_eq!(ctx.flags, vec![(10_007_300, true)]);
        assert_eq!(d.want(), MAX_BOTS);
        let mut w = world([E, E]);
        w.place = Some(Place::Roundtable);
        d.step(50, GameMode::BattleRoyale, &w, &mut ctx);
        assert!(d.take_over());
        assert!(!d.take_over(), "once");
        assert_eq!(d.want(), 0, "the config count is back");
    }

    #[test]
    fn every_minute_the_farthest_bot_is_brought_near_local() {
        let mut d = BotFill::new(2, 29);
        d.stage = Stage::Done;
        d.rounds = 1;
        d.kept.push(Kept::new(0xA0));
        d.kept.push(Kept::new(0xA1));
        let mut w = world([E, E]);
        w.local_pos = Some(([0.0, 0.0, 0.0], true));
        let bot = |handle, x: f32| BotChr {
            handle,
            chr: 0x7000 + handle as usize,
            team: 29,
            npc_id: 600_040_000,
            hp: 700,
            max_hp: 240,
            pos: Some([x, 0.0, 0.0]),
            attacker: 0,
        };
        w.bot_chrs = vec![bot(0xA0, 300.0), bot(0xA1, 180.0)];
        let mut ctx = Ctx::default();
        d.step(0, GameMode::Offline, &w, &mut ctx);
        assert!(
            !ctx.bot_writes
                .iter()
                .any(|b| matches!(b, BotWrite::Hunt { .. })),
            "not before a minute"
        );
        let mut ctx = Ctx::default();
        d.step(HUNT_EVERY_MS, GameMode::Offline, &w, &mut ctx);
        assert!(
            ctx.bot_writes.contains(&BotWrite::Hunt {
                chr: 0x70A0,
                handle: 0xA0
            }),
            "{:?}",
            ctx.bot_writes
        );
        // A bot already near is left alone.
        w.bot_chrs = vec![bot(0xA0, 100.0), bot(0xA1, 120.0)];
        let mut ctx = Ctx::default();
        d.step(HUNT_EVERY_MS * 2, GameMode::Offline, &w, &mut ctx);
        assert!(!ctx
            .bot_writes
            .iter()
            .any(|b| matches!(b, BotWrite::Hunt { .. })));
    }

    fn arena_bot(handle: u64, hp: u32, x: f32) -> BotChr {
        BotChr {
            handle,
            chr: 0x7000 + handle as usize,
            team: 29,
            npc_id: 600_040_000,
            hp,
            max_hp: hp / 3,
            pos: Some([x, 3.0, 0.0]),
            attacker: 0,
        }
    }

    fn arena_in(round_ends: u32, resets: u32, decided: u32) -> ArenaIn {
        ArenaIn {
            on: true,
            size: 1,
            placed: true,
            events: BotEvents {
                round_ends,
                resets,
                decided,
            },
        }
    }

    fn puts(ctx: &Ctx) -> Vec<(u64, [f32; 3])> {
        ctx.bot_writes
            .iter()
            .filter_map(|w| match w {
                BotWrite::Put { handle, to, .. } => Some((*handle, *to)),
                _ => None,
            })
            .collect()
    }

    fn hps(ctx: &Ctx) -> Vec<(u64, u32)> {
        ctx.bot_writes
            .iter()
            .filter_map(|w| match w {
                BotWrite::Hp { handle, hp, .. } => Some((*handle, *hp)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_arena_asks_for_one_level_13_bot_only_once_local_is_on_its_spawn() {
        let mut d = BotFill::new(0, 29);
        let mut ctx = Ctx::default();
        d.set_arena(ArenaIn {
            placed: false,
            ..arena_in(0, 0, 0)
        });
        d.step(0, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        assert!(ctx.flags.is_empty(), "the warp is not done yet");
        d.set_arena(arena_in(0, 0, 0));
        d.step(50, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        assert_eq!(d.want(), 1);
        // The table brings 2: one kept, the other left for the clear (not sent anywhere).
        let mut ctx = Ctx::default();
        let mut w = world([0xA0, 0xA1]);
        w.bot_chrs = vec![arena_bot(0xA0, 900, -14.0), arena_bot(0xA1, 900, -15.0)];
        d.step(100, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(ctx.bot_writes[0], BotWrite::ForgetSlot(0));
        assert!(!ctx
            .bot_writes
            .iter()
            .any(|b| matches!(b, BotWrite::Spread { .. })));
        let mut ctx = Ctx::default();
        d.step(100 + ROUND_GAP_MS, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(ctx.flags, vec![(super::super::FLAG_CLEAR, true)]);
        assert_eq!(d.stage, Stage::Done);
        // Not BR rules: no camps, no hunt, no 5 bots.
        assert_eq!(d.want(), 1);
    }

    /// One bot kept and seen at LOCAL's side; returns the director at `t` = 2 s.
    fn arena_with_bot(w: &mut World) -> BotFill {
        let mut d = BotFill::new(0, 29);
        d.set_arena(arena_in(0, 0, 0));
        let mut ctx = Ctx::default();
        d.step(0, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        *w = world([0xA0, 0xA1]);
        w.bot_chrs = vec![arena_bot(0xA0, 900, -14.0)];
        d.step(50, GameMode::BattleRoyale, w, &mut ctx);
        d.step(50 + ROUND_GAP_MS, GameMode::BattleRoyale, w, &mut ctx);
        assert_eq!(d.stage, Stage::Done);
        d
    }

    #[test]
    fn the_arena_bot_goes_to_its_spawn_and_is_ready_there() {
        let mut w = World::default();
        let mut d = arena_with_bot(&mut w);
        assert_eq!(
            puts(&{
                let mut ctx = Ctx::default();
                d.step(2_000, GameMode::BattleRoyale, &w, &mut ctx);
                ctx
            }),
            Vec::<(u64, [f32; 3])>::new(),
            "put once, at the first sight"
        );
        let b = d.arena_bot().unwrap();
        assert_eq!((b.handle, b.full_hp, b.ready), (0xA0, 900, false));
        w.bot_chrs[0].pos = Some([15.0, 3.0, 0.5]);
        d.step(2_050, GameMode::BattleRoyale, &w, &mut Ctx::default());
        assert!(d.arena_bot().unwrap().ready);
        assert!(
            d.roster().contains("every bot on its spawn"),
            "{}",
            d.roster()
        );
    }

    #[test]
    fn a_new_arena_bot_is_put_on_spawn_b() {
        let mut d = BotFill::new(0, 29);
        d.set_arena(arena_in(0, 0, 0));
        let mut ctx = Ctx::default();
        d.step(0, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        let mut w = world([0xA0, 0xA1]);
        w.bot_chrs = vec![arena_bot(0xA0, 900, -14.0)];
        d.step(50, GameMode::BattleRoyale, &w, &mut Ctx::default());
        // Kept on this tick, put on its spawn on the next.
        let mut ctx = Ctx::default();
        d.step(100, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(puts(&ctx), vec![(0xA0, ARENA_BOT_SPAWN)]);
        assert_eq!(
            ARENA_BOT_SPAWN,
            crate::game::arenacard::spawn_for(Some(1), 0).0
        );
    }

    #[test]
    fn a_living_arena_bot_is_parked_at_the_round_end_then_healed_and_put_back() {
        let mut w = World::default();
        let mut d = arena_with_bot(&mut w);
        w.bot_chrs[0].pos = Some([15.0, 3.0, 0.0]);
        d.step(2_000, GameMode::BattleRoyale, &w, &mut Ctx::default());
        assert!(d.arena_bot().unwrap().ready);
        // LOCAL went down (or the time ran out): the bot leaves.
        w.bot_chrs[0].hp = 600;
        d.set_arena(arena_in(1, 0, 0));
        let mut ctx = Ctx::default();
        d.step(3_000, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(puts(&ctx), vec![(0xA0, ARENA_PARK)]);
        assert!(!d.arena_bot().unwrap().ready);
        w.bot_chrs[0].pos = Some(ARENA_PARK);
        d.step(3_050, GameMode::BattleRoyale, &w, &mut Ctx::default());
        assert!(!d.arena_bot().unwrap().ready, "parked is not ready");
        // The reset: full HP (the spawn's), back on B.
        d.set_arena(arena_in(1, 1, 0));
        let mut ctx = Ctx::default();
        d.step(8_000, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(hps(&ctx), vec![(0xA0, 900)]);
        assert_eq!(puts(&ctx), vec![(0xA0, ARENA_BOT_SPAWN)]);
        assert!(ctx.flags.is_empty(), "no new bot");
        w.bot_chrs[0].pos = Some([15.0, 3.0, 0.0]);
        d.step(8_050, GameMode::BattleRoyale, &w, &mut Ctx::default());
        assert!(d.arena_bot().unwrap().ready);
    }

    #[test]
    fn a_dead_arena_bot_is_replaced_at_the_reset_and_the_ending_removes_the_last() {
        let mut w = World::default();
        let mut d = arena_with_bot(&mut w);
        w.local_handle = Some(0xF0);
        // LOCAL kills it: no killfeed in the Arena; the round ends.
        w.bot_chrs[0].hp = 0;
        w.bot_chrs[0].attacker = 0xF0;
        let mut ctx = Ctx::default();
        d.step(2_000, GameMode::BattleRoyale, &w, &mut ctx);
        assert!(ctx.kills.is_empty());
        d.set_arena(arena_in(1, 0, 0));
        let mut ctx = Ctx::default();
        d.step(2_050, GameMode::BattleRoyale, &w, &mut ctx);
        assert!(puts(&ctx).is_empty(), "a dead bot is not parked");
        assert!(ctx.flags.is_empty(), "no new bot before the reset");
        // The reset: a new table round.
        d.set_arena(arena_in(1, 1, 0));
        let mut ctx = Ctx::default();
        d.step(7_000, GameMode::BattleRoyale, &w, &mut ctx);
        assert!(d.arena_bot().is_none(), "no bot until the new one is seen");
        d.step(7_050, GameMode::BattleRoyale, &w, &mut ctx);
        d.step(7_100 + ROUND_GAP_MS, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        // The new bot: kept, put on B; the old one stays dead.
        let mut w2 = world([0xB0, 0xB1]);
        w2.local_handle = Some(0xF0);
        w2.bot_chrs = vec![w.bot_chrs[0], arena_bot(0xB0, 880, -14.0)];
        d.step(
            8_000 + ROUND_GAP_MS,
            GameMode::BattleRoyale,
            &w2,
            &mut Ctx::default(),
        );
        let mut ctx = Ctx::default();
        d.step(8_050 + ROUND_GAP_MS, GameMode::BattleRoyale, &w2, &mut ctx);
        assert_eq!(puts(&ctx), vec![(0xB0, ARENA_BOT_SPAWN)]);
        assert_eq!(d.arena_bot().unwrap().handle, 0xB0);
        // The old bot coming back to life is put at 0 HP.
        w2.bot_chrs[0].hp = 300;
        let mut ctx = Ctx::default();
        d.step(9_000 + ROUND_GAP_MS, GameMode::BattleRoyale, &w2, &mut ctx);
        assert!(hps(&ctx).contains(&(0xA0, 0)), "{:?}", ctx.bot_writes);
        // The match is decided while the new bot lives: it is removed, and no more bots come.
        w2.bot_chrs[0].hp = 0;
        d.set_arena(arena_in(2, 1, 1));
        let mut ctx = Ctx::default();
        d.step(20_000, GameMode::BattleRoyale, &w2, &mut ctx);
        assert!(hps(&ctx).contains(&(0xB0, 0)), "{:?}", ctx.bot_writes);
        assert!(d.roster().contains("done (match decided)"));
        w2.bot_chrs[1].hp = 0;
        d.set_arena(arena_in(2, 2, 1));
        let mut ctx = Ctx::default();
        d.step(25_000, GameMode::BattleRoyale, &w2, &mut ctx);
        d.step(30_000, GameMode::BattleRoyale, &w2, &mut ctx);
        assert!(ctx.flags.is_empty(), "no bot after the match");
    }

    #[test]
    fn duo_and_trio_places_enemies_first_then_allies_next_to_local() {
        let duo = arena_roles(2);
        assert_eq!(
            duo,
            vec![
                Role {
                    ally: false,
                    slot: 0
                },
                Role {
                    ally: false,
                    slot: 1
                },
                Role {
                    ally: true,
                    slot: 1
                },
            ]
        );
        assert_eq!(
            arena_roles(1),
            vec![Role {
                ally: false,
                slot: 0
            }]
        );
        let trio = arena_roles(3);
        assert_eq!(trio.len(), 5);
        assert_eq!(role_spawn(trio[0]), ARENA_BOT_SPAWN, "Solo's bot place");
        // Allies stand on LOCAL's side (A, x < 0), enemies on B; nobody on another's spawn or on LOCAL's (team A slot 0).
        let local = crate::game::arenacard::spawn_for(None, 0).0;
        let mut spots: Vec<[f32; 3]> = trio.iter().map(|r| role_spawn(*r)).collect();
        for (r, p) in trio.iter().zip(&spots) {
            assert_eq!(r.ally, p[0] < 0.0, "{r:?} at {p:?}");
            assert_ne!(*p, local);
        }
        spots.dedup();
        assert_eq!(spots.len(), 5);
        assert_ne!(role_park(trio[0]), role_park(trio[3]), "parked apart");
    }

    /// ARENA-6: a Duo match against bots with its three bots kept (two table rounds) and seen.
    fn arena_duo(w: &mut World) -> BotFill {
        let mut d = BotFill::new(0, 29);
        d.set_arena(ArenaIn {
            size: 2,
            ..arena_in(0, 0, 0)
        });
        let mut ctx = Ctx::default();
        d.step(0, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        assert_eq!(d.want(), 3);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        // The game brings new bots next to LOCAL; here far from every spawn, so none counts as standing on one before its put.
        *w = world([0xA0, 0xA1]);
        w.bot_chrs = vec![arena_bot(0xA0, 900, 60.0), arena_bot(0xA1, 900, 61.0)];
        d.step(50, GameMode::BattleRoyale, w, &mut Ctx::default());
        // The second table round brings the third bot (its slot mate is not needed).
        let mut ctx = Ctx::default();
        d.step(50 + ROUND_GAP_MS, GameMode::BattleRoyale, w, &mut ctx);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        let mut w2 = world([0xA2, 0xA3]);
        w2.bot_chrs = w.bot_chrs.clone();
        w2.bot_chrs.push(arena_bot(0xA2, 900, 62.0));
        w2.bot_chrs.push(arena_bot(0xA3, 900, 63.0));
        d.step(
            100 + ROUND_GAP_MS,
            GameMode::BattleRoyale,
            &w2,
            &mut Ctx::default(),
        );
        d.step(
            100 + 2 * ROUND_GAP_MS,
            GameMode::BattleRoyale,
            &w2,
            &mut Ctx::default(),
        );
        assert_eq!(d.stage, Stage::Done);
        w2.bot_chrs.pop();
        *w = w2;
        d
    }

    #[test]
    fn a_duo_against_bots_keeps_two_enemies_on_team_6_and_one_ally_on_team_2() {
        let mut w = World::default();
        let mut d = arena_duo(&mut w);
        let roles: Vec<(u64, Option<Role>)> = d.kept.iter().map(|k| (k.handle, k.role)).collect();
        assert_eq!(
            roles,
            vec![
                (
                    0xA0,
                    Some(Role {
                        ally: false,
                        slot: 0
                    })
                ),
                (
                    0xA1,
                    Some(Role {
                        ally: false,
                        slot: 1
                    })
                ),
                (
                    0xA2,
                    Some(Role {
                        ally: true,
                        slot: 1
                    })
                ),
            ]
        );
        let mut ctx = Ctx::default();
        d.step(3_000, GameMode::BattleRoyale, &w, &mut ctx);
        let teams: Vec<(u64, u8)> = ctx
            .bot_writes
            .iter()
            .filter_map(|b| match b {
                BotWrite::Team { handle, team, .. } => Some((*handle, *team)),
                _ => None,
            })
            .collect();
        assert_eq!(
            teams,
            vec![
                (0xA0, ARENA_ENEMY_TEAM),
                (0xA1, ARENA_ENEMY_TEAM),
                (0xA2, ARENA_ALLY_TEAM)
            ]
        );
        let bots = d.arena_bots();
        assert_eq!(bots.iter().filter(|b| b.ally).count(), 1);
        assert_eq!(bots.iter().filter(|b| !b.ally).count(), 2);
        // Ready only once all three stand on their own spawns.
        assert!(!d.arena_all_ready());
        for (i, r) in arena_roles(2).into_iter().enumerate() {
            w.bot_chrs[i].pos = Some(role_spawn(r));
            d.step(
                3_100 + i as u64,
                GameMode::BattleRoyale,
                &w,
                &mut Ctx::default(),
            );
            assert_eq!(d.arena_all_ready(), i == 2, "after bot {i}");
        }
    }

    #[test]
    fn a_duo_reset_heals_the_living_and_replaces_a_dead_ally() {
        let mut w = World::default();
        let mut d = arena_duo(&mut w);
        for (i, r) in arena_roles(2).into_iter().enumerate() {
            w.bot_chrs[i].pos = Some(role_spawn(r));
        }
        d.step(3_000, GameMode::BattleRoyale, &w, &mut Ctx::default());
        assert!(d.arena_all_ready());
        // The ally dies; the round ends: the living enemies are parked on their side, the dead ally is not moved.
        w.bot_chrs[2].hp = 0;
        d.step(3_100, GameMode::BattleRoyale, &w, &mut Ctx::default());
        assert!(d.arena_bots().iter().any(|b| b.ally && b.out));
        d.set_arena(ArenaIn {
            size: 2,
            ..arena_in(1, 0, 0)
        });
        let mut ctx = Ctx::default();
        d.step(3_200, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(puts(&ctx), vec![(0xA0, ARENA_PARK), (0xA1, ARENA_PARK)]);
        // The reset: enemies healed and back; one new bot asked for, which takes the ally's place.
        d.set_arena(ArenaIn {
            size: 2,
            ..arena_in(1, 1, 0)
        });
        let mut ctx = Ctx::default();
        d.step(8_000, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(hps(&ctx), vec![(0xA0, 900), (0xA1, 900)]);
        assert!(!d.arena_all_ready(), "the ally's place is empty");
        let mut ctx = Ctx::default();
        d.step(8_050, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        let mut w2 = world([0xB0, 0xB1]);
        w2.bot_chrs = w.bot_chrs.clone();
        w2.bot_chrs.push(arena_bot(0xB0, 880, -14.0));
        d.step(
            9_000 + ROUND_GAP_MS,
            GameMode::BattleRoyale,
            &w2,
            &mut Ctx::default(),
        );
        let k = d.kept.iter().find(|k| k.handle == 0xB0).unwrap();
        assert_eq!(
            k.role,
            Some(Role {
                ally: true,
                slot: 1
            })
        );
    }

    /// Leonardo's Trio match of 2026-11-25: the bot not needed in the first fill is cleared, and its handle comes back in a slot at the
    /// refill after three enemies died. It is a new bot: kept, so the refill brings all three enemies.
    #[test]
    fn a_trio_refill_keeps_a_new_bot_on_the_handle_of_a_cleared_one() {
        let trio = |r, s, dec| ArenaIn {
            size: 3,
            ..arena_in(r, s, dec)
        };
        let mut d = BotFill::new(0, 29);
        d.set_arena(trio(0, 0, 0));
        let mut ctx = Ctx::default();
        d.step(0, GameMode::BattleRoyale, &world([E, E]), &mut ctx);
        assert_eq!(d.want(), 5);
        // Three table rounds: 2 + 2 + 1 kept; 0xA5 is not needed and stays in slot 1 for the clear.
        let mut chrs = Vec::new();
        let mut t = 0;
        for pair in [[0xA0, 0xA1], [0xA2, 0xA3], [0xA4, 0xA5]] {
            chrs.extend(pair.iter().map(|h| arena_bot(*h, 900, 60.0)));
            let mut w = world(pair);
            w.bot_chrs = chrs.clone();
            t += 50;
            d.step(t, GameMode::BattleRoyale, &w, &mut Ctx::default());
            t += ROUND_GAP_MS;
            d.step(t, GameMode::BattleRoyale, &w, &mut Ctx::default());
        }
        assert_eq!(d.stage, Stage::Done);
        assert_eq!(d.kept.len(), 5);
        // The clear removed 0xA5; the three enemies (0xA0..0xA2) die in round 1.
        chrs.retain(|b| b.handle != 0xA5);
        for b in chrs.iter_mut().take(3) {
            b.hp = 0;
        }
        let mut w = world([E, E]);
        w.bot_chrs = chrs.clone();
        d.step(3_000, GameMode::BattleRoyale, &w, &mut Ctx::default());
        d.set_arena(trio(1, 0, 0));
        d.step(3_100, GameMode::BattleRoyale, &w, &mut Ctx::default());
        d.set_arena(trio(1, 1, 0));
        d.step(8_000, GameMode::BattleRoyale, &w, &mut Ctx::default());
        let mut ctx = Ctx::default();
        d.step(8_050, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        // The refill: the game hands out 0xA5 again with 0xA6; both are new and kept.
        chrs.push(arena_bot(0xA5, 900, 60.0));
        chrs.push(arena_bot(0xA6, 900, 60.0));
        let mut w = world([0xA5, 0xA6]);
        w.bot_chrs = chrs.clone();
        let mut ctx = Ctx::default();
        d.step(8_100, GameMode::BattleRoyale, &w, &mut ctx);
        let forgets: Vec<&BotWrite> = ctx
            .bot_writes
            .iter()
            .filter(|b| matches!(b, BotWrite::ForgetSlot(_)))
            .collect();
        assert_eq!(
            forgets,
            [&BotWrite::ForgetSlot(0), &BotWrite::ForgetSlot(1)]
        );
        // One more round for the third enemy; its slot mate is not needed.
        let mut ctx = Ctx::default();
        d.step(8_100 + ROUND_GAP_MS, GameMode::BattleRoyale, &w, &mut ctx);
        assert_eq!(ctx.flags, vec![(ARENA_TABLE.1, true)]);
        chrs.push(arena_bot(0xA7, 900, 60.0));
        chrs.push(arena_bot(0xA8, 900, 60.0));
        let mut w = world([0xA7, 0xA8]);
        w.bot_chrs = chrs;
        for at in [8_150, 8_200] {
            d.step(
                at + ROUND_GAP_MS,
                GameMode::BattleRoyale,
                &w,
                &mut Ctx::default(),
            );
        }
        let enemies: Vec<u64> = d
            .arena_bots()
            .iter()
            .filter(|b| !b.ally && !b.out)
            .map(|b| b.handle)
            .collect();
        assert_eq!(enemies, vec![0xA5, 0xA6, 0xA7]);
        assert_eq!(d.active(), 5);
    }

    #[test]
    fn the_roundtable_ends_the_expedition() {
        let mut d = BotFill::new(2, 29);
        let mut ctx = Ctx::default();
        d.step(0, GameMode::Offline, &world([E, E]), &mut ctx);
        let mut w = world([E, E]);
        w.place = Some(Place::Roundtable);
        d.step(50, GameMode::Offline, &w, &mut ctx);
        assert_eq!(d.stage, Stage::Idle);
    }
}
