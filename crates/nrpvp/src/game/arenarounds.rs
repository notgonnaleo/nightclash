//! ARENA-2 (dev only): best-of-3 rounds of a 1v1 Arena match (TASKS ARENA-2, PROJECT_SCOPE §Arena, DECISIONS "Arena details").
//! Game 1.03.3 / exe 1.3.3.0.
//!
//! Each game runs the same rules on what it sees and acts only on its own player and its own screen:
//! - **Start.** The screen is black from the arrival (`arenacard`); [`START_FADE_IN_MS`] after LOCAL is placed on its spawn it fades
//!   in and round 1 starts.
//! - **A round ends with a down.** LOCAL downed (`PlayerIns+0x710` = 1, or HP 0) = round lost; the rival downed = round won. While
//!   an Arena match is on, the Battle Royale rules, the rival re-down rule and the revive block stand aside
//!   ([`crate::mm::arena_match`]); the downed rule stays, so a rival's hit never revives a downed player.
//! - **ARENA-HP1: a round ends at 1 HP, normally with no down at all.** For the whole Arena match each game holds its player immortal
//!   (Set Character Immortality 2004:12 on flag [`FLAG_IMMORTAL`], MEMORY_MAP §6): a real hit stops at 1 HP. LOCAL at [`BEATEN_HP`]
//!   or less = round lost, the rival the same = round won. The down and the get-up below stay only as the fallback, should a down
//!   still happen (the get-up after a down broke the next round on real Steam, 2026-11-15 and 2026-11-17).
//! - **Between rounds, behind a black screen.** [`FADE_OUT_MS`] after the down the screen fades to black and the player is frozen
//!   (Fade to Black 2004:77). [`REVIVE_AFTER_MS`] after the down the loser's game asks the game's own get-up the way a teammate's
//!   revive does (the revive bar to 0, every tick until it is up; never the byte `+0x732`, MEMORY_MAP §6). At [`RESET_AFTER_MS`] each game restores its player (Bonfire-like Recovery 2004:47 on flag
//!   10007410, Refill Estus 2004:87 on flag 10007514; the captured consumables back, [`super::arenaloadout::refill_goods`]) and
//!   puts it on its spawn
//!   ([`super::arenacard::place`]). At [`FADE_IN_MS`] the screen comes back, the player is free, the next round starts. A round counts
//!   a down only once both players have been seen up.
//! - **Round time limit ([`ROUND_LIMIT_MS`], ARENA-3).** A round with no down by then goes to the side with more HP in percent of
//!   its maximum (a draw within [`DRAW_WITHIN`]: nobody scores, the round is played again). The **host** judges and publishes the
//!   verdict (`net::bus` topic `ARENA_ROUND`, key = the bout number, value 1 host won / 2 host lost / 3 draw) so both games agree;
//!   the other game takes it, or judges by itself after [`VERDICT_WAIT_MS`] more without one.
//! - **Overtime (ARENA-OT, Leonardo 2026-11-17).** At the limit nobody wins on the spot: for up to [`OVERTIME_MS`] each game drains its
//!   own player's HP ([`OVERTIME_DRAIN_PER_SEC`] of the maximum per second; the bot too in a bot match), so the first to fall to 1 HP
//!   loses. In overtime only the host judges (the downs it sees; both at once = a draw) and sends the verdict; at the end of the
//!   overtime with nobody at 1 HP the HP judgement above decides.
//! - **First to [`ROUNDS_TO_WIN`].** The last round ends the same way (both reset at their spawns), but the screen comes back with the
//!   players still frozen and the endings play: WIN in the winner's game (flag 10007411: banner 4000, 10 s, 7000), LOSE in the
//!   loser's (10007412: banner 4010, 10 s, 7001).
//! - **One ultimate per round ([`UltOnce`], Leonardo 2026-11-17).** Every round starts with LOCAL's gauge full; once it is used it
//!   cannot charge again until the next round's reset.
//! - **ARENA-4, against a bot.** In an Arena match against a bot ([`crate::mm::arena_bots`]) the rival is the bot the bots feature
//!   publishes ([`set_arena_bots`]): down at 0 HP, its HP in percent of the HP it had at its spawn. LOCAL judges the round limit (no
//!   session). The round events ([`bot_events`]) tell the bots feature when to park, heal, replace or remove the bot, and every fade-in
//!   that starts a round waits (at most [`RIVAL_WAIT_MS`]) until the bot stands on its spawn. ARENA-6: Duo / Trio against bots
//!   ([`crate::mm::arena_bots_size`]): the bot allies are LOCAL's teammates and the enemy bots its rivals, judged like players.
//! - **ARENA-6: a beaten player watches its team.** LOCAL beaten while a teammate (player or bot ally) still fights is frozen, loses
//!   collision and gravity (flag [`FLAG_GHOST`]) and is moved to its team's far bench; once the others have that position its
//!   character is disabled (flag [`FLAG_HIDE`]: not drawn on its own game, and its position no longer reaches the others, who keep
//!   seeing it on the bench), then its body follows a living teammate every tick, so its camera shows that teammate (as the Battle
//!   Royale spectator does with a dead body); the spectator's switch inputs change who is watched ([`watch_stage`], [`HIDE_BODY`],
//!   console `arena_hide`). Both flags are off again at the reset.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::game::world::World;

/// Rounds a player needs to win the match (best of 3).
pub(crate) const ROUNDS_TO_WIN: u8 = 2;
/// ARENA-3: a round with no down ends after this long (PROJECT_SCOPE §Arena: 3 minutes, to be tuned).
pub(crate) const ROUND_LIMIT_MS: u64 = 180_000;
/// ARENA-CLOCK: a message warns the players when this much of the round is left (only with a longer limit).
pub(crate) const WARN_LEFT_MS: u64 = 60_000;
/// ARENA-OT: after the limit the round goes on this long with both players' HP draining ([`OVERTIME_DRAIN_PER_SEC`]); the first at
/// 1 HP loses; nobody by then = the HP judgement ([`judge`]).
pub(crate) const OVERTIME_MS: u64 = 60_000;
/// ARENA-OT: the share of its maximum HP each player loses per second in overtime (full HP lasts 50 s).
pub(crate) const OVERTIME_DRAIN_PER_SEC: f32 = 0.02;
/// The non-host game waits this long past the limit for the host's verdict, then judges by itself.
pub(crate) const VERDICT_WAIT_MS: u64 = 3_000;
/// HP fractions closer than this are a draw.
pub(crate) const DRAW_WITHIN: f32 = 0.005;
/// From LOCAL placed on its spawn to the first fade-in (the move and the camera turn are done by then).
pub(crate) const START_FADE_IN_MS: u64 = 1_000;
/// From the down: fade to black (0.5 s) and freeze.
pub(crate) const FADE_OUT_MS: u64 = 1_000;
/// From the down: the loser's get-up.
pub(crate) const REVIVE_AFTER_MS: u64 = 2_000;
/// From the down: the reset at the spawns (the get-up animation, ~3.5 s, has about finished).
pub(crate) const RESET_AFTER_MS: u64 = 5_000;
/// From the down: the screen comes back.
pub(crate) const FADE_IN_MS: u64 = 6_000;
/// ARENA-HP1: a player at this HP or less has lost the round (immortality stops a hit at 1 HP).
pub(crate) const BEATEN_HP: u32 = 1;
/// ARENA-4: a round start waits at most this long for the bot on its spawn (then goes on without it).
pub(crate) const RIVAL_WAIT_MS: u64 = 30_000;
/// The screen comes back only once LOCAL's move to its spawn is written, waiting at most this long past [`FADE_IN_MS`]. A player
/// frozen mid-attack at the round end has a tilted rotation: the move is refused until it is forced after [`PLACE_FORCE_AFTER_MS`],
/// which is also the gap from the reset to the fade-in, so the forced move landed 0.1-0.3 s into the next round (2v2 of 2026-11-25:
/// a 7.4 m and a 33.8 m jump in sight, after "fight!").
pub(crate) const PLACE_WAIT_MS: u64 = 3_000;
/// The get-up is asked the way a teammate's revive does it: the revive bar `+0x714` to 0 and the pause `+0x71C` up, every tick while
/// LOCAL is downed in the get-up window. The player script then plays its revival, the animation sets "revive completed" and the
/// game's state 2 follows (MEMORY_MAP §6). Never the byte `+0x732` itself: it makes the code lead, and a write later than ~3.5 s after
/// the down skipped the get-up (state 1 -> 0, no state 2) and the player died ~3 s later with a level lost (real Steam 2026-11-15,
/// 8 of 8 on one PC 2026-11-16; the bar route 16 of 16, testing-log 2026-11-16-arena-getup-spike). The log line is written once per
/// this long.
pub(crate) const REVIVE_EVERY_MS: u64 = 2_500;

/// Test aid: the get-up asked from this long after the down (0 = [`REVIVE_AFTER_MS`]); console `arena_getup`.
static REVIVE_AFTER_OVERRIDE_MS: AtomicU64 = AtomicU64::new(0);
/// Test aid: rounds to win the match (0 = [`ROUNDS_TO_WIN`]); a big value keeps a test match going for many downs. Console `arena_bo`.
static ROUNDS_TO_WIN_OVERRIDE: AtomicU8 = AtomicU8::new(0);
/// LOCAL's bar is being emptied for the get-up (the downed rule must not restore it as if a rival had hit it).
static BAR_GETUP: AtomicBool = AtomicBool::new(false);
/// After a get-up that still went 1 -> 0 with no state 2 (the game may kill the player a few seconds later): until then LOCAL counts
/// as neither up nor down (no round starts, no down counts), then LOCAL is put on its spawn again.
static SKIP_GUARD: Mutex<Option<Instant>> = Mutex::new(None);
const SKIP_GUARD_MS: u64 = 12_000;

fn rounds_to_win() -> u8 {
    match ROUNDS_TO_WIN_OVERRIDE.load(Ordering::Relaxed) {
        0 => ROUNDS_TO_WIN,
        n => n,
    }
}

fn revive_after_ms() -> u64 {
    match REVIVE_AFTER_OVERRIDE_MS.load(Ordering::Relaxed) {
        0 => REVIVE_AFTER_MS,
        n => n.min(RESET_AFTER_MS - 500),
    }
}

/// The downed rule leaves LOCAL's bar alone while the Arena get-up empties it.
pub(crate) fn bar_getup_active() -> bool {
    BAR_GETUP.load(Ordering::Relaxed)
}

/// `arena_getup [after_ms]`: when the get-up is asked after the down (test aid; 0 = the default).
pub(crate) fn getup_cmd(args: &[String]) -> Result<String, String> {
    if let Some(a) = args.first() {
        let ms: u64 = a.parse().map_err(|_| "after_ms must be a number")?;
        REVIVE_AFTER_OVERRIDE_MS.store(ms, Ordering::Relaxed);
    }
    Ok(format!(
        "arena_getup: asked {} ms after the down (revive bar to 0)",
        revive_after_ms()
    ))
}

/// `arena_bo [n]`: rounds to win (0 = the default 2).
pub(crate) fn bo_cmd(args: &[String]) -> Result<String, String> {
    if let Some(a) = args.first() {
        let n: u8 = a.parse().map_err(|_| "n must be 0..255")?;
        ROUNDS_TO_WIN_OVERRIDE.store(n, Ordering::Relaxed);
    }
    Ok(format!("arena_bo: {} round(s) to win", rounds_to_win()))
}

/// Whether a get-up write is due now (pure): the first one at once, another only [`REVIVE_EVERY_MS`] after the last.
pub(crate) fn revive_due(since_last_ms: Option<u64>) -> bool {
    since_last_ms.is_none_or(|ms| ms >= REVIVE_EVERY_MS)
}
/// A reset move refused (a tilted rotation mid-attack) is retried every tick, then written over the rotation after this long.
const PLACE_FORCE_AFTER_MS: u64 = 1_000;

/// Our per-game event flags (`gamedata/emevd-nights.json`, `gamedata/emevd-arena.json`).
const FLAG_HEAL: u32 = 10_007_410;
const FLAG_REFILL_ESTUS: u32 = 10_007_514;
const FLAG_WIN: u32 = 10_007_411;
const FLAG_LOSE: u32 = 10_007_412;
const FLAG_FADE_OUT: u32 = 10_007_515;
const FLAG_FADE_IN: u32 = 10_007_516;
const FLAG_FADE_IN_FROZEN: u32 = 10_007_517;
/// ARENA-4: while ON this game's player holds SpEffect 8970061 (event 99100919): offline a solo player at 0 HP in the arena dies at
/// once ("YOU DIED", `mi-arena-bot` 2026-11-10); with it 0 HP is a down. A down uses it up: OFF at each reset, ON at each round start.
const FLAG_NEAR_DEATH: u32 = 10_007_519;
/// ARENA-HP1: while ON this game's player is immortal (event 99100920, re-applied every 0.5 s): a hit stops at 1 HP, no down.
const FLAG_IMMORTAL: u32 = 10_007_520;
/// ARENA-6: while ON this game's player has no collision and no gravity (event 99100921: Change Character Collision State 2004:39 and
/// Set Character Gravity 2004:10, re-applied every 0.5 s), so a spectating body that follows a teammate neither pushes it nor falls.
const FLAG_GHOST: u32 = 10_007_521;
/// ARENA-6: while ON this game's player is disabled (event 99100922: Change Character Enable State 2004:5). `[VERIFIED:
/// mi-arena-hide-lab 2026-11-25, two instances]`: on its own game the body is not drawn and the camera still moves with it (a `tp`);
/// the other game keeps drawing it, frozen at the position it had when it was disabled (its position updates stop until it is
/// enabled again). So the body is put on the far bench first and disabled only once the others have that position ([`watch_stage`]).
const FLAG_HIDE: u32 = 10_007_522;
/// ARENA-6: from the bench move to the hide: the bench position reaches the other games (they draw the body there from then on).
pub(crate) const HIDE_AFTER_MS: u64 = 1_500;
/// ARENA-6: from the hide to the first follow move: the hide event has run (it applies on the flag's change; re-applied every 0.5 s).
pub(crate) const FOLLOW_AFTER_MS: u64 = 700;
/// ARENA-6: whether a spectating body is also disabled ([`FLAG_HIDE`]); console `arena_hide 0|1`.
static HIDE_BODY: AtomicBool = AtomicBool::new(true);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// No Arena match, or LOCAL not on its spawn yet.
    Off,
    /// LOCAL stands on its spawn behind the black screen since `at_ms`.
    Starting { at_ms: u64 },
    /// A round is on; `armed` once every player has been seen up (a down before that is the last round's).
    Fighting { armed: bool },
    /// The round ended with a down at `at_ms`; `lost` = LOCAL went down; `stage` counts the steps done (fade out, reset).
    RoundOver { lost: bool, at_ms: u64, stage: u8 },
    /// The match is decided; nothing more until the match is over.
    MatchOver,
}

/// What the main loop does (pure output of [`Rounds::step`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Do {
    /// ARENA-OT: overtime: drain this game's player's HP (and the bot's), every tick while the round is undecided.
    Drain,
    Message(String),
    FadeOut,
    FadeIn,
    FadeInFrozen,
    /// ARENA-5 / ARENA-6: LOCAL is beaten while a teammate still fights: frozen (screen on); it watches a living teammate (its body
    /// follows one) or, with nobody to follow, sits on its team's bench.
    Spectate,
    Revive,
    Reset,
    /// The host's verdict on a timed-out bout (key, outcome from the host's side) for the other game.
    Verdict(u8, Outcome),
    Win,
    Lose,
}

/// One player as the rules see it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Seen {
    /// The round is lost: at [`BEATEN_HP`] or less, or downed.
    pub down: bool,
    pub up: bool,
    /// Really downed (state 1): only then is a get-up asked (a player at 1 HP and standing needs none).
    pub downed: bool,
}

impl Seen {
    pub(crate) fn of(state: u8, hp: u32) -> Seen {
        Seen {
            down: state == 1 || hp <= BEATEN_HP,
            up: state == 0 && hp > BEATEN_HP,
            downed: state == 1,
        }
    }
}

/// How a bout ended for LOCAL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Won,
    Lost,
    Draw,
}

impl Outcome {
    /// The bus value from the host's side: bits 0..8 the host's outcome (1 won, 2 lost, 3 draw); ARENA-5: bits 8.. the host's roster
    /// team + 1 (0 = unknown), so the host's teammates take the host's outcome and only the rivals take the opposite.
    pub(crate) fn wire(self, host_team: Option<u8>) -> u32 {
        let o = match self {
            Outcome::Won => 1,
            Outcome::Lost => 2,
            Outcome::Draw => 3,
        };
        o | host_team.map_or(0, |t| (u32::from(t) + 1) << 8)
    }

    /// This game's outcome from the host's bus value (pure). `my_team`: this game's roster team. The host's team unknown on either
    /// side = the 1v1 rule (the other game is the rival).
    pub(crate) fn from_host_wire(v: u32, my_team: Option<u8>) -> Option<Outcome> {
        let host = match v & 0xFF {
            1 => Outcome::Won,
            2 => Outcome::Lost,
            3 => Outcome::Draw,
            _ => return None,
        };
        let host_team = (v >> 8).checked_sub(1);
        let same_team = matches!((host_team, my_team), (Some(h), Some(m)) if h == u32::from(m));
        Some(match (same_team, host) {
            (true, o) | (false, o @ Outcome::Draw) => o,
            (false, Outcome::Won) => Outcome::Lost,
            (false, Outcome::Lost) => Outcome::Won,
        })
    }
}

/// ARENA-5 (pure): a side has lost the round: it has players and every one of them is beaten. A side nobody sees (left, not loaded)
/// never loses (the 1v1 rule: no rival seen = no round won).
pub(crate) fn all_beaten(side: &[Seen]) -> bool {
    !side.is_empty() && side.iter().all(|s| s.down)
}

/// ARENA-5 (pure): every player of a side is up (and the side has players).
pub(crate) fn all_up(side: &[Seen]) -> bool {
    !side.is_empty() && side.iter().all(|s| s.up)
}

/// ARENA-5 (pure): a side's HP for the time limit: the mean of its players' HP fractions (equal team sizes: the sum of PROJECT_SCOPE
/// §Arena; a leaver's team keeps its mean, as it plays on short-handed). `None` = nobody seen.
pub(crate) fn side_hp(fractions: &[f32]) -> Option<f32> {
    (!fractions.is_empty()).then(|| fractions.iter().sum::<f32>() / fractions.len() as f32)
}

/// ARENA-6 (pure): LOCAL watches its team now: beaten while a teammate fought on, from then until the reset (the reset puts it back on
/// its spawn, where it must stand on the floor again).
pub(crate) fn spectating_now(r: &Rounds) -> bool {
    r.spectating
        && match r.phase {
            Phase::Fighting { .. } => true,
            Phase::RoundOver { stage, .. } => stage < 2,
            Phase::Starting { .. } | Phase::Off | Phase::MatchOver => false,
        }
}

/// ARENA-6: where a watching LOCAL is: going to the bench, seen there by everyone, hidden there, following a teammate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WatchStage {
    Parking,
    Parked,
    Hidden,
    Following,
}

/// ARENA-6 (pure): the stage from the time since the bench move was written (`None` = not yet) and whether the body is hidden at all
/// (`arena_hide 0`: no hide, it follows at once, drawn on the teammate on every game).
pub(crate) fn watch_stage(parked_ms: Option<u64>, hide: bool) -> WatchStage {
    match parked_ms {
        None => WatchStage::Parking,
        Some(_) if !hide => WatchStage::Following,
        Some(t) if t < HIDE_AFTER_MS => WatchStage::Parked,
        Some(t) if t < HIDE_AFTER_MS + FOLLOW_AFTER_MS => WatchStage::Hidden,
        Some(_) => WatchStage::Following,
    }
}

/// ARENA-6 (pure): the teammate to watch among `cands` (id, world position): the current one while it is still a candidate, the next
/// / previous by id on a switch input (wraps), else the lowest id. `None` = nobody to watch.
pub(crate) fn pick_watch(
    cur: Option<u64>,
    cands: &[(u64, [f32; 3])],
    next: bool,
    prev: bool,
) -> Option<(u64, [f32; 3])> {
    let mut ids: Vec<u64> = cands.iter().map(|c| c.0).collect();
    ids.sort_unstable();
    let n = ids.len();
    let id = match (
        cur.and_then(|c| ids.iter().position(|i| *i == c)),
        next,
        prev,
    ) {
        _ if n == 0 => return None,
        (Some(i), true, _) => ids[(i + 1) % n],
        (Some(i), false, true) => ids[(i + n - 1) % n],
        (Some(i), false, false) => ids[i],
        (None, _, _) => ids[0],
    };
    cands.iter().find(|c| c.0 == id).copied()
}

/// `arena_hide [0|1]`: whether a spectating body is also disabled (hidden) or only a ghost (no collision, no gravity).
pub(crate) fn hide_cmd(args: &[String]) -> Result<String, String> {
    if let Some(a) = args.first() {
        let on = match a.as_str() {
            "0" => false,
            "1" => true,
            _ => return Err("arena_hide takes 0 or 1".into()),
        };
        HIDE_BODY.store(on, Ordering::Relaxed);
    }
    Ok(format!(
        "arena_hide: a spectating body is {}",
        if HIDE_BODY.load(Ordering::Relaxed) {
            "disabled (hidden) and a ghost"
        } else {
            "only a ghost (no collision, no gravity), still drawn"
        }
    ))
}

/// ARENA-3: what the round timer needs from the world this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Timer {
    /// This game is the session host (it judges).
    pub host: bool,
    /// `(LOCAL, rival)` HP as a fraction of each one's maximum.
    pub hp: Option<(f32, f32)>,
    /// The host's verdict for the current bout, as LOCAL's outcome.
    pub verdict: Option<Outcome>,
    /// The round limit; 0 = [`ROUND_LIMIT_MS`] (the console's `arena_limit` sets a shorter one for tests).
    pub limit_ms: u64,
    /// ARENA-4: the rival (a bot) is not on its spawn yet: a fade-in that starts a round waits (at most [`RIVAL_WAIT_MS`]).
    pub rival_missing: bool,
    /// LOCAL's move back to its spawn is not written yet: the fade-in after a reset waits (at most [`PLACE_WAIT_MS`]).
    pub unplaced: bool,
}

/// Console override of the round limit (ms; 0 = the default).
static LIMIT_OVERRIDE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The round limit in force (the console override, else [`ROUND_LIMIT_MS`]).
pub(crate) fn round_limit_ms() -> u64 {
    match LIMIT_OVERRIDE_MS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => ROUND_LIMIT_MS,
        v => v,
    }
}

/// `arena_limit [secs]`: set (0 = back to the default) or show the round limit.
pub(crate) fn limit_cmd(args: &[String]) -> Result<String, String> {
    use std::sync::atomic::Ordering;
    if let Some(a) = args.first() {
        let s: u64 = a.parse().map_err(|_| "seconds must be a number")?;
        LIMIT_OVERRIDE_MS.store(s * 1000, Ordering::Relaxed);
    }
    let v = LIMIT_OVERRIDE_MS.load(Ordering::Relaxed);
    Ok(format!(
        "arena_limit: {} s{}",
        if v == 0 { ROUND_LIMIT_MS } else { v } / 1000,
        if v == 0 {
            " (default)"
        } else {
            " (console override)"
        }
    ))
}

/// The time-limit judgement (pure): more HP in percent wins, a near tie or unknown HP is a draw.
pub(crate) fn judge(hp: Option<(f32, f32)>) -> Outcome {
    match hp {
        Some((l, r)) if (l - r).abs() >= DRAW_WITHIN => {
            if l > r {
                Outcome::Won
            } else {
                Outcome::Lost
            }
        }
        _ => Outcome::Draw,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rounds {
    pub won: u8,
    pub lost: u8,
    pub phase: Phase,
    /// When the current round's fight started (its fade-in).
    pub fight_since_ms: u64,
    /// Bouts ended so far, draws included (the verdict key).
    pub bouts: u8,
    /// ARENA-CLOCK: the "one minute left" message was shown in this round.
    pub warned: bool,
    /// ARENA-OT: the overtime message was shown in this round.
    pub overtime_said: bool,
    /// ARENA-5 / ARENA-6: LOCAL was beaten in this round while a teammate fought on, and watches until the round ends.
    pub spectating: bool,
}

impl Default for Rounds {
    fn default() -> Self {
        Rounds {
            won: 0,
            lost: 0,
            phase: Phase::Off,
            fight_since_ms: 0,
            bouts: 0,
            warned: false,
            overtime_said: false,
            spectating: false,
        }
    }
}

impl Rounds {
    fn round_no(&self) -> u8 {
        self.won + self.lost
    }

    /// ARENA-LV: rounds are being played (from the first round's start to the last reset; not before, not after the match).
    pub(crate) fn in_play(&self) -> bool {
        matches!(
            self.phase,
            Phase::Starting { .. } | Phase::Fighting { .. } | Phase::RoundOver { .. }
        )
    }

    fn decided(&self) -> bool {
        self.won >= rounds_to_win() || self.lost >= rounds_to_win()
    }

    /// One tick with no round timer input (pure; the tests of the down rules).
    #[cfg(test)]
    pub(crate) fn step(
        &mut self,
        now_ms: u64,
        on: bool,
        local: Seen,
        rival: Option<Seen>,
    ) -> Vec<Do> {
        self.step_timed(now_ms, on, local, rival, Timer::default())
    }

    /// One 1v1 tick (pure; the tests of the 1v1 rules). `rival` is `None` when this game sees no rival.
    #[cfg(test)]
    pub(crate) fn step_timed(
        &mut self,
        now_ms: u64,
        on: bool,
        local: Seen,
        rival: Option<Seen>,
        timer: Timer,
    ) -> Vec<Do> {
        self.step_teams(now_ms, on, local, &[], rival.as_slice(), timer)
    }

    /// One tick (pure). `on`: an Arena match is on and LOCAL stands in the arena. ARENA-5: `mates` are LOCAL's teammates and `rivals`
    /// the other team, as this game sees them (a leaver is simply missing: the team plays on short-handed). A round is lost when
    /// LOCAL and every teammate are beaten, won when every rival is; LOCAL beaten alone watches its team ([`Do::Spectate`]).
    pub(crate) fn step_teams(
        &mut self,
        now_ms: u64,
        on: bool,
        local: Seen,
        mates: &[Seen],
        rivals: &[Seen],
        timer: Timer,
    ) -> Vec<Do> {
        if !on {
            *self = Rounds::default();
            return Vec::new();
        }
        let local_out = local.down || self.spectating;
        let we_lost = local_out && mates.iter().all(|s| s.down);
        let they_lost = all_beaten(rivals);
        match self.phase {
            Phase::Off => {
                self.phase = Phase::Starting { at_ms: now_ms };
                Vec::new()
            }
            Phase::Starting { at_ms } => {
                let t = now_ms.saturating_sub(at_ms);
                if t < START_FADE_IN_MS
                    || (timer.rival_missing && t < START_FADE_IN_MS + RIVAL_WAIT_MS)
                {
                    return Vec::new();
                }
                self.phase = Phase::Fighting { armed: false };
                self.fight_since_ms = now_ms;
                self.warned = false;
                self.overtime_said = false;
                vec![Do::FadeIn, Do::Message("Round 1: fight!".into())]
            }
            Phase::Fighting { armed: false } => {
                if local.up && mates.iter().all(|s| s.up) && all_up(rivals) {
                    self.phase = Phase::Fighting { armed: true };
                }
                Vec::new()
            }
            Phase::Fighting { armed: true } => {
                let mut out = Vec::new();
                let t = now_ms.saturating_sub(self.fight_since_ms);
                let limit = if timer.limit_ms == 0 {
                    ROUND_LIMIT_MS
                } else {
                    timer.limit_ms
                };
                // ARENA-5 / ARENA-6: beaten while a teammate fights on: out of this round, watching the team.
                if local_out && !we_lost && !self.spectating {
                    self.spectating = true;
                    out.push(Do::Spectate);
                    out.push(Do::Message(
                        "You are out of this round. Watching your team (mouse buttons or D-pad switch)".into(),
                    ));
                }
                let (outcome, timed) = if t < limit {
                    // The round proper: each game judges the downs it sees.
                    if we_lost {
                        (Outcome::Lost, false)
                    } else if they_lost {
                        (Outcome::Won, false)
                    } else if let Some(v) = timer.verdict {
                        (v, true)
                    } else {
                        if !self.warned && limit > WARN_LEFT_MS && t >= limit - WARN_LEFT_MS {
                            self.warned = true;
                            out.push(Do::Message("One minute left!".into()));
                        }
                        return out;
                    }
                } else {
                    // ARENA-OT: overtime. Each game drains its own player (and the bot); the host alone judges, so a fall to 1 HP on
                    // both games at about the same moment cannot give both a lost round.
                    if !self.overtime_said {
                        self.overtime_said = true;
                        out.push(Do::Message(
                            "Overtime! HP drains until someone falls".into(),
                        ));
                    }
                    let capped = t >= limit + OVERTIME_MS;
                    let by_downs = match (we_lost, they_lost) {
                        (true, true) => Some(Outcome::Draw),
                        (true, false) => Some(Outcome::Lost),
                        (false, true) => Some(Outcome::Won),
                        (false, false) => None,
                    };
                    let o = if let Some(v) = timer.verdict {
                        Some(v)
                    } else if timer.host {
                        let o = by_downs.or_else(|| capped.then(|| judge(timer.hp)));
                        if let Some(o) = o {
                            out.push(Do::Verdict(self.bouts + 1, o));
                        }
                        o
                    } else if t >= limit + OVERTIME_MS + VERDICT_WAIT_MS {
                        Some(by_downs.unwrap_or_else(|| judge(timer.hp)))
                    } else {
                        None
                    };
                    let Some(o) = o else {
                        if !capped {
                            out.push(Do::Drain);
                        }
                        return out;
                    };
                    (o, true)
                };
                self.bouts += 1;
                match outcome {
                    Outcome::Won => self.won += 1,
                    Outcome::Lost => self.lost += 1,
                    Outcome::Draw => {}
                }
                self.phase = Phase::RoundOver {
                    lost: outcome == Outcome::Lost,
                    at_ms: now_ms,
                    stage: 0,
                };
                let what = match outcome {
                    Outcome::Won => "you won",
                    Outcome::Lost => "you lost",
                    Outcome::Draw => "draw, played again",
                };
                out.push(Do::Message(format!(
                    "Round {}: {}{what} ({}-{})",
                    self.round_no() + u8::from(outcome == Outcome::Draw),
                    if timed { "time! " } else { "" },
                    self.won,
                    self.lost
                )));
                out
            }
            Phase::RoundOver { lost, at_ms, stage } => {
                let t = now_ms.saturating_sub(at_ms);
                let mut out = Vec::new();
                let mut stage = stage;
                if stage == 0 && t >= FADE_OUT_MS {
                    stage = 1;
                    out.push(Do::FadeOut);
                }
                // A downed LOCAL gets up whoever won (ARENA-5: a teammate of the winners may be the one downed).
                if stage <= 1 && (revive_after_ms()..RESET_AFTER_MS).contains(&t) && local.downed {
                    out.push(Do::Revive);
                }
                if stage == 1 && t >= RESET_AFTER_MS {
                    stage = 2;
                    out.push(Do::Reset);
                }
                let rival_wait =
                    !self.decided() && timer.rival_missing && t < FADE_IN_MS + RIVAL_WAIT_MS;
                let place_wait = timer.unplaced && t < FADE_IN_MS + PLACE_WAIT_MS;
                if stage == 2 && t >= FADE_IN_MS && !rival_wait && !place_wait {
                    if self.decided() {
                        self.phase = Phase::MatchOver;
                        out.push(Do::FadeInFrozen);
                        out.push(if self.won >= rounds_to_win() {
                            Do::Win
                        } else {
                            Do::Lose
                        });
                    } else {
                        self.phase = Phase::Fighting { armed: false };
                        self.fight_since_ms = now_ms;
                        self.warned = false;
                        self.overtime_said = false;
                        self.spectating = false;
                        out.push(Do::FadeIn);
                        out.push(Do::Message(format!(
                            "Round {}: fight!",
                            self.round_no() + 1
                        )));
                    }
                    return out;
                }
                self.phase = Phase::RoundOver { lost, at_ms, stage };
                out
            }
            Phase::MatchOver => Vec::new(),
        }
    }
}

/// A gauge at this fraction of the charge needed or more counts as full.
const ULT_FULL_WITHIN: f32 = 0.999;

/// One ultimate per round (DECISIONS "One ultimate per round", Leonardo 2026-11-17; replaces "Ultimate only in the last round"). Pure:
/// a round starts with the gauge filled; once it has been full and drops (the ultimate was used), it may fall further (an ultimate that
/// drains as it plays is never cut short) but never rise again (no recharge from hits) until [`UltOnce::default`] at the next reset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UltOnce {
    /// Seen full this round.
    full: bool,
    /// Used this round: the lowest charge seen since.
    low: Option<f32>,
}

impl UltOnce {
    /// The charge to write now (`None` = leave the gauge alone), from the charge now and the charge needed.
    pub(crate) fn step(&mut self, now: f32, needed: f32) -> Option<f32> {
        if let Some(low) = self.low {
            if now > low {
                return Some(low);
            }
            self.low = Some(now);
            return None;
        }
        if now >= needed * ULT_FULL_WITHIN {
            self.full = true;
            None
        } else if self.full {
            self.low = Some(now);
            None
        } else {
            Some(needed)
        }
    }

    pub(crate) fn used(&self) -> bool {
        self.low.is_some()
    }
}

/// LOCAL's ultimate this round (main loop only).
static ULT: Mutex<UltOnce> = Mutex::new(UltOnce {
    full: false,
    low: None,
});

/// ARENA-4: the types shared with the bots feature (defined there: that feature is in both builds, this module is dev only).
use crate::features::bots::fill::{ArenaBot, BotEvents};

/// ARENA-4 / ARENA-6: the bots of the match and whether every place has its bot on its spawn.
static ARENA_BOTS: Mutex<(Vec<ArenaBot>, bool)> = Mutex::new((Vec::new(), false));
static BOT_EVENTS: Mutex<BotEvents> = Mutex::new(BotEvents {
    round_ends: 0,
    resets: 0,
    decided: 0,
});

/// ARENA-4 / ARENA-6 (bots feature, every tick): the bots the Arena match has (allies and enemies), and whether all stand on their
/// spawns since the last round end.
pub(crate) fn set_arena_bots(bots: Vec<ArenaBot>, all_ready: bool) {
    *lock(&ARENA_BOTS) = (bots, all_ready);
}

/// ARENA-4 (bots feature): the round events so far.
pub(crate) fn bot_events() -> BotEvents {
    *lock(&BOT_EVENTS)
}

static STATE: Mutex<Option<(Rounds, Instant)>> = Mutex::new(None);
/// SB-ARENA: LOCAL's rounds `(won, lost)` while an Arena match is on (`None` = no Arena match); the scoreboard shows them instead of
/// kills and deaths (an Arena round ends with a down, which the Battle Royale kill count never sees).
pub(crate) fn score() -> Option<(u8, u8)> {
    lock(&STATE)
        .as_ref()
        .filter(|(r, _)| r.phase != Phase::Off)
        .map(|(r, _)| (r.won, r.lost))
}

/// ARENA-5: session ids of the other players seen beaten in the current round (main loop only; cleared outside an armed round).
static BEATEN_IDS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
/// ARENA-5: a bench move is waiting (since when; main loop only).
static BENCH_SINCE: Mutex<Option<Instant>> = Mutex::new(None);
/// ARENA-6: whom LOCAL watches (main loop only).
#[derive(Clone, Copy, Debug, Default)]
struct Watch {
    /// When the bench move was written (the stages count from it).
    parked_at: Option<Instant>,
    /// The watched teammate (session id, or a bot ally's handle).
    target: Option<u64>,
    /// Nobody to follow: sent to the bench.
    benched: bool,
    /// A follow move failed (logged once per target).
    failed: bool,
    /// The bench move was written (the hide waits for it: a body hidden anywhere else stays there in the others' view).
    bench_ok: bool,
}
static WATCH: Mutex<Watch> = Mutex::new(Watch {
    parked_at: None,
    target: None,
    benched: false,
    failed: false,
    bench_ok: false,
});
/// ARENA-6: the values last written to [`FLAG_GHOST`] / [`FLAG_HIDE`] (`None` = never), and whether a failed write was logged.
static GHOST: Mutex<Option<bool>> = Mutex::new(None);
static HIDE: Mutex<Option<bool>> = Mutex::new(None);
static GHOST_WARNED: AtomicBool = AtomicBool::new(false);
static HIDE_WARNED: AtomicBool = AtomicBool::new(false);

/// ARENA-6: write one of our saved switch flags when [`saved_flag_write_due`] (the first tick of a session too, so a game closed while
/// spectating never leaves the player a hidden ghost); a failed write (title screen, load) is logged once and tried again next tick.
fn write_switch(
    base: usize,
    flag: u32,
    want: bool,
    last: &Mutex<Option<bool>>,
    warned: &AtomicBool,
    what: &str,
) {
    let mut l = lock(last);
    if !saved_flag_write_due(*l, want) {
        return;
    }
    match crate::game::flags::set(base, flag, want) {
        Ok(_) => {
            tracing::info!("arena_rounds: {what} {}", if want { "on" } else { "off" });
            *l = Some(want);
            warned.store(false, Ordering::Relaxed);
        }
        Err(e) => {
            if !warned.swap(true, Ordering::Relaxed) {
                tracing::warn!("arena_rounds: flag {flag} ({what}) not set yet: {e}");
            }
        }
    }
}
/// ARENA-5: `(teammates, rivals)` last logged.
static SIDES: Mutex<(usize, usize)> = Mutex::new((usize::MAX, usize::MAX));

/// ARENA-5 / ARENA-6: another player or a bot of the match as this tick sees it.
#[derive(Clone, Copy, Debug)]
struct Member {
    /// Session id (a player) or handle (a bot).
    id: u64,
    seen: Seen,
    /// World position (`None` = not read).
    pos: Option<[f32; 3]>,
    /// HP as a fraction of its maximum (a bot: of its spawn HP), for the time limit.
    frac: Option<f32>,
}

/// ARENA-5 (pure): a player once beaten in the round stays beaten (`ids` holds who was); others as seen.
pub(crate) fn sticky(ids: &mut Vec<u64>, id: u64, s: Seen) -> Seen {
    if s.down && !ids.contains(&id) {
        ids.push(id);
    }
    if ids.contains(&id) {
        Seen {
            down: true,
            up: false,
            ..s
        }
    } else {
        s
    }
}

/// The host's verdicts received this match: `(bout key, LOCAL's outcome)`.
static VERDICTS: Mutex<Vec<(u32, Outcome)>> = Mutex::new(Vec::new());

/// A player's HP as a fraction of its maximum (`PlayerGameData+0x10` / `+0x14`, MEMORY_MAP §29).
fn hp_fraction(player: usize) -> Option<f32> {
    let pgd = super::mem::read::<usize>(player + crate::game::layout::PLAYER_GAMEDATA)
        .filter(|p| super::mem::is_ptr(*p))?;
    let hp = super::mem::read::<u32>(pgd + 0x10)?;
    let max = super::mem::read::<u32>(pgd + 0x14).filter(|m| *m > 0)?;
    Some(hp as f32 / max as f32)
}
/// When the last get-up write was made, and since when a reset move is waiting (main loop only).
static LAST_REVIVE: Mutex<Option<Instant>> = Mutex::new(None);
static PLACE_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// ARENA-4 (pure): whether LOCAL should hold SpEffect 8970061 ([`FLAG_NEAR_DEATH`]): in a match against a bot, from the arrival to the
/// reset after each down (the reset clears it, the next round start gives it back), never once the match is decided.
pub(crate) fn near_death_wanted(vs_bot: bool, on: bool, phase: Phase) -> bool {
    vs_bot
        && on
        && match phase {
            Phase::Starting { .. } | Phase::Fighting { .. } => true,
            Phase::RoundOver { stage, .. } => stage < 2,
            Phase::Off | Phase::MatchOver => false,
        }
}

/// Whether a saved Arena flag must be written now (pure). Our flag group is saved with the character (TRAPS: flag 10007520), so a
/// session's first tick writes the wanted value even when it is OFF: a game closed during an Arena match keeps the flag ON in the save,
/// and the player was immortal in a later Battle Royale match (2026-11-20). Afterwards only changes are written.
pub(crate) fn saved_flag_write_due(last: Option<bool>, want: bool) -> bool {
    last != Some(want)
}

/// The value last written to [`FLAG_NEAR_DEATH`] (main loop only; `None` = never written).
static NEAR_DEATH: Mutex<Option<bool>> = Mutex::new(None);
/// The value last written to [`FLAG_IMMORTAL`] (main loop only; `None` = never written).
static IMMORTAL: Mutex<Option<bool>> = Mutex::new(None);

fn set_flag(base: usize, flag: u32) {
    if let Err(e) = crate::game::flags::set(base, flag, true) {
        tracing::warn!("arena_rounds: flag {flag} not set: {e}");
    }
}

/// LOCAL's last state byte and HP, when the current down started, and whether state 2 was seen since.
struct Diag {
    state: u8,
    hp: u32,
    down_at: Option<Instant>,
    saw_getup: bool,
}
static DIAG: Mutex<Option<Diag>> = Mutex::new(None);

/// One line per change of LOCAL's state byte, and a death (HP 0 while state 0), timed from the down; a 1 -> 0 with HP above 0
/// and no state 2 seen is the skipped get-up.
fn getup_diag(on: bool, state: u8, hp: u32) {
    let mut d = lock(&DIAG);
    if !on {
        *d = None;
        return;
    }
    let Some(p) = d.as_mut() else {
        *d = Some(Diag {
            state,
            hp,
            down_at: None,
            saw_getup: false,
        });
        return;
    };
    let since = |p: &Diag| {
        p.down_at.map_or("-".to_string(), |t| {
            format!("+{} ms", t.elapsed().as_millis())
        })
    };
    if state != p.state {
        if state == 1 {
            p.down_at = Some(Instant::now());
            p.saw_getup = false;
        }
        if state == 2 {
            p.saw_getup = true;
        }
        let skip = p.state == 1 && state == 0 && !p.saw_getup && hp > 0;
        if skip {
            *lock(&SKIP_GUARD) = Some(Instant::now());
        }
        tracing::info!(
            "arena_getup: LOCAL state {} -> {state}, hp {hp}, {} after the down{}",
            p.state,
            since(p),
            if skip {
                " SKIP: up with no get-up (state 2 never seen); this player's rounds wait until it is safe"
            } else {
                ""
            }
        );
        p.state = state;
    }
    if state == 0 && hp == 0 && p.hp > 0 {
        // A death in the Arena (only after a skipped get-up): the comeback takes ~7 s more, the guard waits for it.
        *lock(&SKIP_GUARD) = Some(Instant::now());
        tracing::info!(
            "arena_getup: DEATH: LOCAL hp 0 in state 0, {} after the down; this player's rounds wait for the comeback",
            since(p)
        );
    }
    p.hp = hp;
}

/// Main loop, every tick (after `arenacard::tick`).
pub(crate) fn tick(base: usize, world: &World) {
    let on = crate::mm::arena_match() && super::arenacard::placed();
    let mut st = lock(&STATE);
    let (rounds, start) = st.get_or_insert_with(|| (Rounds::default(), Instant::now()));
    let now_ms = start.elapsed().as_millis() as u64;
    let vs_bot = crate::mm::arena_bots();
    // ARENA-6: the spectator's switches, before anything else (also at the Roundtable: they are saved with the character). The hide
    // waits for the stage the last tick reached (the body on the bench, seen there by the others).
    let watching = on && spectating_now(rounds);
    let hide = HIDE_BODY.load(Ordering::Relaxed);
    let stage = watch_stage(
        lock(&WATCH)
            .parked_at
            .map(|t| t.elapsed().as_millis() as u64),
        hide,
    );
    write_switch(
        base,
        FLAG_GHOST,
        watching,
        &GHOST,
        &GHOST_WARNED,
        "spectator ghost (no collision, no gravity)",
    );
    write_switch(
        base,
        FLAG_HIDE,
        watching && hide && matches!(stage, WatchStage::Hidden | WatchStage::Following),
        &HIDE,
        &HIDE_WARNED,
        "spectator body hidden",
    );
    let Some(me) = world.fighters.iter().flatten().find(|f| f.local).copied() else {
        return;
    };
    let (local_addr, mut local_seen, local_raw) =
        (me.addr, Seen::of(me.state, me.hp), Some((me.state, me.hp)));
    // ARENA-5: the other players split by team (`team_key`, equal for teammates on every game); against a bot there are none. A player
    // beaten in this round stays beaten until it ends (a heal on the bench never brings a player back), on every game alike.
    let mut beaten = lock(&BEATEN_IDS);
    if rounds.phase != (Phase::Fighting { armed: true }) {
        beaten.clear();
    }
    let mut mates: Vec<Member> = Vec::new();
    let mut rivals: Vec<Member> = Vec::new();
    for (i, f) in world.fighters.iter().enumerate() {
        let Some(f) = f.filter(|f| !f.local && !vs_bot) else {
            continue;
        };
        let m = Member {
            id: f.id,
            seen: sticky(&mut beaten, f.id, Seen::of(f.state, f.hp)),
            pos: world.positions.get(i).copied().flatten(),
            frac: hp_fraction(f.addr),
        };
        if f.team == me.team {
            mates.push(m);
        } else {
            rivals.push(m);
        }
    }
    // ARENA-4 / ARENA-6: against bots the bot allies are the teammates and the enemy bots the rivals (out at 0 HP; HP in percent of
    // the HP each had at its spawn).
    let (arena_bots, bots_ready) = if vs_bot {
        lock(&ARENA_BOTS).clone()
    } else {
        (Vec::new(), false)
    };
    for b in &arena_bots {
        let chr = world.bot(b.handle);
        let hp = chr.filter(|_| !b.out).map_or(0, |c| c.hp);
        let m = Member {
            id: b.handle,
            seen: sticky(&mut beaten, b.handle, Seen::of(0, hp)),
            pos: chr.and_then(|c| c.pos),
            frac: (b.full_hp > 0).then(|| (hp as f32 / b.full_hp as f32).min(1.0)),
        };
        if b.ally {
            mates.push(m);
        } else {
            rivals.push(m);
        }
    }
    drop(beaten);
    if !on {
        BAR_GETUP.store(false, Ordering::Relaxed);
        *lock(&SKIP_GUARD) = None;
    }
    if let Some((state, hp)) = local_raw {
        getup_diag(on, state, hp);
    }
    // After a skipped get-up (or the death that can follow it): no round starts and no down counts until the guard ends, then LOCAL
    // goes back to its spawn (a comeback puts the player elsewhere).
    {
        let mut g = lock(&SKIP_GUARD);
        if let Some(t) = *g {
            if (t.elapsed().as_millis() as u64) < SKIP_GUARD_MS {
                local_seen = Seen::default();
            } else {
                *g = None;
                *lock(&PLACE_SINCE) = Some(Instant::now());
                tracing::info!("arena_getup: guard over; back to the spawn");
            }
        }
    }
    // ARENA-3: the host's verdicts on timed-out bouts (key = bout number). ARENA-5: the value names the host's team.
    let my_team = crate::mm::local_roster_team();
    let mut verdicts = lock(&VERDICTS);
    if !on {
        verdicts.clear();
    }
    for r in crate::net::bus::take(crate::net::bus::topic::ARENA_ROUND) {
        if let Some(o) = Outcome::from_host_wire(r.fact.value, my_team) {
            tracing::info!(
                "arena_rounds: the host's verdict for bout {} (0x{:X}): {o:?} for us (team {my_team:?})",
                r.fact.key,
                r.fact.value
            );
            verdicts.push((r.fact.key, o));
        }
    }
    let next_bout = u32::from(rounds.bouts) + 1;
    // The time-limit HP: each side's mean HP fraction (ARENA-5; 1v1: the one player's; bots in percent of their spawn HP).
    let theirs: Vec<f32> = rivals.iter().filter_map(|m| m.frac).collect();
    let rival_hp = side_hp(&theirs);
    let mine: Vec<f32> = std::iter::once(hp_fraction(local_addr))
        .chain(mates.iter().map(|m| m.frac))
        .flatten()
        .collect();
    let timer = Timer {
        host: vs_bot || super::defeatdrop::is_host(base),
        hp: side_hp(&mine).zip(rival_hp),
        verdict: verdicts.iter().find(|v| v.0 == next_bout).map(|v| v.1),
        limit_ms: LIMIT_OVERRIDE_MS.load(std::sync::atomic::Ordering::Relaxed),
        rival_missing: vs_bot && !bots_ready,
        unplaced: lock(&PLACE_SINCE).is_some(),
    };
    drop(verdicts);
    let before = rounds.phase;
    let mate_seen: Vec<Seen> = mates.iter().map(|m| m.seen).collect();
    let rival_seen: Vec<Seen> = rivals.iter().map(|r| r.seen).collect();
    let dos = rounds.step_teams(now_ms, on, local_seen, &mate_seen, &rival_seen, timer);
    if !on {
        *lock(&PLACE_SINCE) = None;
        *lock(&BENCH_SINCE) = None;
    }
    if on {
        let mut sides = lock(&SIDES);
        if *sides != (mates.len(), rivals.len()) {
            *sides = (mates.len(), rivals.len());
            tracing::info!(
                "arena_rounds: sides: LOCAL + {} teammate(s) vs {} rival(s) (roster team {my_team:?}, slot {:?})",
                mates.len(),
                rivals.len(),
                crate::mm::local_roster_slot()
            );
        }
    } else {
        *lock(&SIDES) = (usize::MAX, usize::MAX);
    }
    if std::mem::discriminant(&rounds.phase) != std::mem::discriminant(&before) {
        if matches!(rounds.phase, Phase::RoundOver { .. }) {
            lock(&BOT_EVENTS).round_ends += 1;
        }
        tracing::info!(
            "arena_rounds: {before:?} -> {:?} (won {}, lost {})",
            rounds.phase,
            rounds.won,
            rounds.lost
        );
    }
    if before == (Phase::Fighting { armed: false })
        && rounds.phase == (Phase::Fighting { armed: true })
    {
        tracing::info!("arena_rounds: everyone up: a down counts from now");
        // Both games are surely in the arena now: our equipment once more, so the rival draws our real weapons.
        if !vs_bot {
            super::arenaloadout::resend(base);
        }
    }
    // ARENA-4: offline, 0 HP is a down only with SpEffect 8970061.
    let want = near_death_wanted(vs_bot, on, rounds.phase);
    let mut nd = lock(&NEAR_DEATH);
    if saved_flag_write_due(*nd, want) {
        match crate::game::flags::set(base, FLAG_NEAR_DEATH, want) {
            Ok(_) => {
                tracing::info!(
                    "arena_rounds: SpEffect 8970061 (0 HP downs) {}",
                    if want { "given" } else { "taken back" }
                );
                *nd = Some(want);
            }
            Err(e) => tracing::warn!("arena_rounds: flag {FLAG_NEAR_DEATH} not set: {e}"),
        }
    }
    drop(nd);
    // ARENA-HP1: immortal for the whole Arena match (a hit stops at 1 HP = the round end), mortal again once it is over.
    let mut imm = lock(&IMMORTAL);
    if saved_flag_write_due(*imm, on) {
        match crate::game::flags::set(base, FLAG_IMMORTAL, on) {
            Ok(_) => {
                tracing::info!(
                    "arena_rounds: immortality {} (a round ends at {BEATEN_HP} HP)",
                    if on { "on" } else { "off" }
                );
                *imm = Some(on);
            }
            Err(e) => tracing::warn!("arena_rounds: flag {FLAG_IMMORTAL} not set: {e}"),
        }
    }
    drop(imm);
    // ARENA-OT: overtime drains this game's player and, in a bot match, the bots.
    let chrs: Vec<usize> = std::iter::once(local_addr)
        .chain(
            arena_bots
                .iter()
                .filter(|b| !b.out)
                .filter_map(|b| world.bot(b.handle))
                .map(|c| c.chr),
        )
        .collect();
    super::arenaclock::drain(dos.contains(&Do::Drain), &chrs);
    for d in dos {
        match d {
            Do::Drain => {}
            Do::Message(m) => {
                tracing::info!("arena_rounds: {m}");
                crate::game::gametoast::show(&m);
            }
            Do::FadeOut => set_flag(base, FLAG_FADE_OUT),
            Do::FadeIn => set_flag(base, FLAG_FADE_IN),
            Do::FadeInFrozen => set_flag(base, FLAG_FADE_IN_FROZEN),
            Do::Spectate => {
                // Frozen with the screen on (the match-end fade, from a visible screen: it stays visible), to the far bench first; the
                // watch below hides it there and then moves it.
                set_flag(base, FLAG_FADE_IN_FROZEN);
                *lock(&WATCH) = Watch::default();
                *lock(&BENCH_SINCE) = Some(Instant::now());
                tracing::info!(
                    "arena_rounds: LOCAL beaten, a teammate fights on: frozen, watching the team"
                );
            }
            Do::Revive => {
                // Every tick while LOCAL is downed in the window (repeats are harmless: `+0x732` is never touched).
                BAR_GETUP.store(true, Ordering::Relaxed);
                let r = super::mem::write_bytes(
                    local_addr + crate::game::layout::PLAYER_REVIVE_BAR,
                    &0f32.to_le_bytes(),
                )
                .and_then(|()| {
                    super::mem::write_bytes(
                        local_addr + crate::game::layout::PLAYER_PAUSE,
                        &3.5f32.to_le_bytes(),
                    )
                });
                let mut last = lock(&LAST_REVIVE);
                if revive_due(last.map(|t| t.elapsed().as_millis() as u64)) {
                    *last = Some(Instant::now());
                    match r {
                        Ok(()) => tracing::info!("arena_rounds: get-up asked (revive bar 0)"),
                        Err(e) => tracing::warn!("arena_rounds: get-up write failed: {e}"),
                    }
                }
            }
            Do::Reset => {
                // ARENA-4: the last round's reset already ends the match for the bot (no new bot behind the black screen).
                let mut ev = lock(&BOT_EVENTS);
                if rounds.decided() {
                    ev.decided += 1;
                } else {
                    ev.resets += 1;
                }
                drop(ev);
                BAR_GETUP.store(false, Ordering::Relaxed);
                set_flag(base, FLAG_HEAL);
                set_flag(base, FLAG_REFILL_ESTUS);
                *lock(&BENCH_SINCE) = None;
                *lock(&PLACE_SINCE) = Some(Instant::now());
                tracing::info!("arena_rounds: reset: recovery, estus, back to the spawn");
                // ARENA-LV: a level lost in the round (2026-11-11: level 14 after a revive by the rival) never carries over.
                super::arenaloadout::reapply(base, true);
                // Consumables used in the round come back for the next one (Leonardo, 2026-11-17).
                super::arenaloadout::refill_goods(base);
                // The next round has its own ultimate.
                if !rounds.decided() {
                    *lock(&ULT) = UltOnce::default();
                }
            }
            Do::Verdict(key, o) => {
                let sent = crate::net::bus::publish(
                    crate::net::bus::topic::ARENA_ROUND,
                    u32::from(key),
                    o.wire(my_team),
                );
                tracing::info!(
                    "arena_rounds: bout {key} timed out: {o:?} for the host, team {my_team:?} (published {sent})"
                );
            }
            Do::Win | Do::Lose => {
                set_flag(base, if d == Do::Win { FLAG_WIN } else { FLAG_LOSE });
                tracing::info!(
                    "arena_rounds: match over, {} {}-{}",
                    if d == Do::Win { "WIN" } else { "LOSE" },
                    rounds.won,
                    rounds.lost
                );
            }
        }
    }
    // ARENA-CLOCK: the round's seconds left in the rune counter (the old count back once the match is over).
    super::arenaclock::show(
        local_addr,
        super::arenaclock::seconds_left(rounds, now_ms, round_limit_ms()),
    );
    // ARENA-LV: hold level 15 only while rounds are played (mi-arena-rounds 2026-11-12: after the match, back at the Roundtable,
    // `on` still held for ~30 s and the hold set level 15 on the Roundtable's level 1).
    super::arenaloadout::hold_tick(base, on && rounds.in_play());
    // One ultimate per round: full at each round's start, no recharge once used.
    let mut ult = lock(&ULT);
    if on && rounds.in_play() {
        if let Some((_, now, needed)) = super::ultgauge::read(local_addr) {
            let was_used = ult.used();
            if let Some(v) = ult.step(now, needed) {
                match super::ultgauge::set(local_addr, v / needed) {
                    Ok((b, a)) if a >= needed * ULT_FULL_WITHIN => tracing::info!(
                        "arena_rounds: ultimate filled for this round ({b:.1} -> {a:.1})"
                    ),
                    Ok((b, a)) => tracing::debug!(
                        "arena_rounds: ultimate held after its use ({b:.1} -> {a:.1})"
                    ),
                    Err(e) => tracing::warn!("arena_rounds: ultimate not written: {e}"),
                }
            }
            if ult.used() && !was_used {
                tracing::info!(
                    "arena_rounds: ultimate used ({now:.1} / {needed:.1}); none more this round"
                );
            }
        }
    } else {
        *ult = UltOnce::default();
    }
    drop(ult);
    let mut pending = lock(&PLACE_SINCE);
    if let Some(t) = *pending {
        let force = t.elapsed().as_millis() as u64 >= PLACE_FORCE_AFTER_MS;
        if super::arenacard::place(base, force) || force {
            *pending = None;
        }
    }
    drop(pending);
    // ARENA-6: watching: on the bench (seen there), hidden there, then the body follows a living teammate every tick (the camera goes
    // with it; the others still see it on the bench); nobody left to follow = back to the bench.
    let mut w = lock(&WATCH);
    if watching && w.parked_at.is_none() && w.bench_ok {
        w.parked_at = Some(Instant::now());
        tracing::info!(
            "arena_rounds: on the bench; hidden in {HIDE_AFTER_MS} ms, then following a teammate"
        );
    }
    if watching && stage == WatchStage::Following {
        // A living teammate; once none is left (the round is over) the one watched last, so the camera stays on the team through the
        // fade-out (the last teammate beaten is not hidden: its position is live).
        let up: Vec<(u64, [f32; 3])> = mates
            .iter()
            .filter(|m| !m.seen.down)
            .filter_map(|m| m.pos.map(|p| (m.id, p)))
            .collect();
        let cands: Vec<(u64, [f32; 3])> = if up.is_empty() {
            mates
                .iter()
                .filter(|m| Some(m.id) == w.target)
                .filter_map(|m| m.pos.map(|p| (m.id, p)))
                .collect()
        } else {
            up
        };
        match pick_watch(w.target, &cands, world.spec_next, world.spec_prev) {
            Some((id, p)) => {
                if w.target != Some(id) {
                    tracing::info!("arena_rounds: watching teammate {id:#X}");
                    w.failed = false;
                }
                w.target = Some(id);
                w.benched = false;
                if let Err(e) = super::spawn::teleport(local_addr, base, p) {
                    if !std::mem::replace(&mut w.failed, true) {
                        tracing::warn!("arena_rounds: follow move to {id:#X} failed: {e}");
                    }
                }
            }
            None if !w.benched => {
                w.target = None;
                w.benched = true;
                *lock(&BENCH_SINCE) = Some(Instant::now());
                tracing::info!("arena_rounds: no teammate to watch: to the bench");
            }
            None => {}
        }
    } else if !watching {
        *w = Watch::default();
    }
    drop(w);
    // ARENA-5: the bench move, retried like the reset move (the player may be mid-attack when beaten).
    let mut bench = lock(&BENCH_SINCE);
    if let Some(t) = *bench {
        let force = t.elapsed().as_millis() as u64 >= PLACE_FORCE_AFTER_MS;
        // ARENA-6: retried (forced after a second) until it is written; only then may the body be hidden.
        if super::arenacard::bench(base, force) {
            *bench = None;
            lock(&WATCH).bench_ok = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: Seen = Seen {
        down: false,
        up: true,
        downed: false,
    };
    const DOWN: Seen = Seen {
        down: true,
        up: false,
        downed: true,
    };
    /// ARENA-HP1: standing at 1 HP (immortal): the round is lost, nobody is downed.
    const BEATEN: Seen = Seen {
        down: true,
        up: false,
        downed: false,
    };

    /// Off -> Starting -> fade in, round 1, armed; returns the next free time.
    fn start(r: &mut Rounds) -> u64 {
        assert!(r.step(0, true, UP, Some(UP)).is_empty());
        assert!(r.step(500, true, UP, Some(UP)).is_empty(), "still black");
        assert_eq!(
            r.step(1_000, true, UP, Some(UP)),
            vec![Do::FadeIn, Do::Message("Round 1: fight!".into())]
        );
        assert!(r.step(1_050, true, UP, Some(UP)).is_empty());
        assert_eq!(r.phase, Phase::Fighting { armed: true });
        2_000
    }

    #[test]
    fn a_player_reads_down_from_the_state_byte_or_zero_hp() {
        assert_eq!(Seen::of(0, 240), UP);
        assert_eq!(Seen::of(1, 0), DOWN);
        assert_eq!(Seen::of(1, 50), DOWN);
        assert_eq!(Seen::of(0, 0), BEATEN);
        assert_eq!(
            Seen::of(0, BEATEN_HP),
            BEATEN,
            "ARENA-HP1: 1 HP ends the round"
        );
        assert_eq!(Seen::of(0, BEATEN_HP + 1), UP);
        assert_eq!(Seen::of(2, 120), Seen::default(), "getting up: neither");
    }

    #[test]
    fn a_round_lost_at_one_hp_asks_no_get_up() {
        let mut r = Rounds::default();
        let t = start(&mut r);
        assert_eq!(
            r.step(t, true, BEATEN, Some(UP)),
            vec![Do::Message("Round 1: you lost (0-1)".into())]
        );
        assert_eq!(r.step(t + 1_000, true, BEATEN, Some(UP)), vec![Do::FadeOut]);
        for ms in [2_000, 3_000, 4_500] {
            assert!(r.step(t + ms, true, BEATEN, Some(UP)).is_empty(), "{ms}");
        }
        assert_eq!(r.step(t + 5_000, true, BEATEN, Some(UP)), vec![Do::Reset]);
        assert_eq!(
            r.step(t + 6_000, true, UP, Some(UP)),
            vec![Do::FadeIn, Do::Message("Round 2: fight!".into())]
        );
        r.step(t + 6_050, true, UP, Some(UP));
        assert_eq!(r.phase, Phase::Fighting { armed: true });
    }

    #[test]
    fn rounds_are_in_play_only_between_the_start_and_the_match_end() {
        let mut r = Rounds::default();
        assert!(!r.in_play(), "off");
        for phase in [
            Phase::Starting { at_ms: 0 },
            Phase::Fighting { armed: true },
            Phase::RoundOver {
                lost: true,
                at_ms: 0,
                stage: 2,
            },
        ] {
            r.phase = phase;
            assert!(r.in_play(), "{phase:?}");
        }
        r.phase = Phase::MatchOver;
        assert!(!r.in_play(), "back toward the Roundtable");
    }

    #[test]
    fn a_round_end_fades_out_resets_and_fades_back_in() {
        let mut r = Rounds::default();
        let t = start(&mut r);
        assert_eq!(
            r.step(t, true, UP, Some(DOWN)),
            vec![Do::Message("Round 1: you won (1-0)".into())]
        );
        assert!(r.step(t + 900, true, UP, Some(DOWN)).is_empty());
        assert_eq!(r.step(t + 1_000, true, UP, Some(DOWN)), vec![Do::FadeOut]);
        assert!(
            r.step(t + 2_500, true, UP, Some(DOWN)).is_empty(),
            "the winner never revives"
        );
        assert_eq!(r.step(t + 5_000, true, UP, Some(UP)), vec![Do::Reset]);
        assert!(r.step(t + 5_500, true, UP, Some(UP)).is_empty());
        assert_eq!(
            r.step(t + 6_000, true, UP, Some(UP)),
            vec![Do::FadeIn, Do::Message("Round 2: fight!".into())]
        );
    }

    #[test]
    fn the_loser_gets_up_behind_the_black_screen() {
        let mut r = Rounds::default();
        let t = start(&mut r);
        r.step(t, true, DOWN, Some(UP));
        assert_eq!((r.won, r.lost), (0, 1));
        assert_eq!(r.step(t + 1_000, true, DOWN, Some(UP)), vec![Do::FadeOut]);
        assert!(
            r.step(t + 1_900, true, DOWN, Some(UP)).is_empty(),
            "not before 2 s"
        );
        assert_eq!(r.step(t + 2_000, true, DOWN, Some(UP)), vec![Do::Revive]);
        assert_eq!(
            r.step(t + 2_050, true, DOWN, Some(UP)),
            vec![Do::Revive],
            "asked while down (the loop spaces the writes)"
        );
        assert!(
            r.step(t + 3_000, true, Seen::default(), Some(UP))
                .is_empty(),
            "getting up"
        );
        assert_eq!(r.step(t + 5_000, true, UP, Some(UP)), vec![Do::Reset]);
    }

    #[test]
    fn a_second_get_up_write_never_lands_inside_the_first_get_up() {
        // Real Steam 2026-11-12: the get-up shows as state 2 about 1 s after the write; a second write then broke it.
        assert!(revive_due(None));
        assert!(!revive_due(Some(1_000)));
        assert!(
            !revive_due(Some(1_100)),
            "the first get-up is still starting"
        );
        assert!(revive_due(Some(REVIVE_EVERY_MS)));
        // A retry still fits before the reset when the first write did nothing.
        const { assert!(REVIVE_AFTER_MS + REVIVE_EVERY_MS < RESET_AFTER_MS) };
    }

    #[test]
    fn the_last_round_resets_too_then_the_ending_plays_with_the_players_frozen() {
        let mut r = Rounds::default();
        let mut t = start(&mut r);
        for round in 1..=2u8 {
            r.step(t, true, UP, Some(DOWN));
            r.step(t + 1_000, true, UP, Some(DOWN));
            assert_eq!(
                r.step(t + 5_000, true, UP, Some(UP)),
                vec![Do::Reset],
                "round {round}"
            );
            if round == 1 {
                r.step(t + 6_000, true, UP, Some(UP));
                r.step(t + 6_050, true, UP, Some(UP));
                t += 10_000;
            }
        }
        assert_eq!((r.won, r.lost), (2, 0));
        assert_eq!(
            r.step(t + 6_000, true, UP, Some(UP)),
            vec![Do::FadeInFrozen, Do::Win]
        );
        assert_eq!(r.phase, Phase::MatchOver);
        assert!(
            r.step(t + 9_000, true, UP, Some(DOWN)).is_empty(),
            "decided once"
        );
    }

    #[test]
    fn the_ultimate_is_filled_once_per_round_and_never_recharges_after_its_use() {
        let mut u = UltOnce::default();
        assert_eq!(u.step(0.0, 1000.0), Some(1000.0), "a round starts full");
        assert_eq!(u.step(1000.0, 1000.0), None, "full: left alone");
        assert!(!u.used());
        assert_eq!(u.step(1000.0, 1000.0), None, "unused: left alone");
        assert_eq!(u.step(0.0, 1000.0), None, "used");
        assert!(u.used());
        assert_eq!(
            u.step(35.0, 1000.0),
            Some(0.0),
            "a hit's recharge is taken back"
        );
        assert_eq!(u.step(0.0, 1000.0), None);
        assert_eq!(
            u.step(1000.0, 1000.0),
            Some(0.0),
            "never full again this round"
        );
        let mut u = UltOnce::default();
        assert_eq!(
            u.step(0.0, 1000.0),
            Some(1000.0),
            "the next round's reset: full again"
        );
    }

    #[test]
    fn an_ultimate_that_drains_while_it_plays_is_not_cut_short() {
        let mut u = UltOnce::default();
        u.step(1000.0, 1000.0);
        assert_eq!(u.step(700.0, 1000.0), None);
        assert_eq!(u.step(300.0, 1000.0), None, "falling: left alone");
        assert_eq!(
            u.step(320.0, 1000.0),
            Some(300.0),
            "rising: held at the lowest"
        );
        assert_eq!(u.step(0.0, 1000.0), None);
        assert_eq!(u.step(10.0, 1000.0), Some(0.0));
    }

    #[test]
    fn two_lost_rounds_lose() {
        let mut r = Rounds::default();
        let mut t = start(&mut r);
        let mut last = Vec::new();
        for _ in 0..2 {
            r.step(t, true, DOWN, Some(UP));
            r.step(t + 1_000, true, DOWN, Some(UP));
            r.step(t + 5_000, true, UP, Some(UP));
            last = r.step(t + 6_000, true, UP, Some(UP));
            r.step(t + 6_050, true, UP, Some(UP));
            t += 10_000;
        }
        assert_eq!(last, vec![Do::FadeInFrozen, Do::Lose]);
        assert_eq!(r.phase, Phase::MatchOver);
    }

    #[test]
    fn a_down_left_over_from_the_last_round_does_not_count() {
        let mut r = Rounds::default();
        let t = start(&mut r);
        r.step(t, true, UP, Some(DOWN));
        r.step(t + 1_000, true, UP, Some(DOWN));
        r.step(t + 5_000, true, UP, Some(DOWN));
        r.step(t + 6_000, true, UP, Some(DOWN));
        assert_eq!(r.phase, Phase::Fighting { armed: false });
        assert!(
            r.step(t + 6_100, true, UP, Some(DOWN)).is_empty(),
            "the rival still reads down: not armed"
        );
        assert_eq!(r.won, 1);
    }

    #[test]
    fn the_end_of_the_match_clears_the_score() {
        let mut r = Rounds::default();
        let t = start(&mut r);
        r.step(t, true, UP, Some(DOWN));
        assert!(r.step(t + 100, false, UP, Some(UP)).is_empty());
        assert_eq!(r, Rounds::default());
    }

    fn timed(host: bool, hp: Option<(f32, f32)>, verdict: Option<Outcome>) -> Timer {
        Timer {
            host,
            hp,
            verdict,
            limit_ms: 0,
            rival_missing: false,
            unplaced: false,
        }
    }

    #[test]
    fn against_a_bot_local_holds_the_near_death_speffect_from_each_round_start_to_the_reset() {
        let p = |stage| Phase::RoundOver {
            lost: true,
            at_ms: 0,
            stage,
        };
        assert!(near_death_wanted(true, true, Phase::Starting { at_ms: 0 }));
        assert!(near_death_wanted(
            true,
            true,
            Phase::Fighting { armed: false }
        ));
        assert!(
            near_death_wanted(true, true, p(1)),
            "downed: keep it until the reset"
        );
        assert!(
            !near_death_wanted(true, true, p(2)),
            "the reset takes it back"
        );
        assert!(!near_death_wanted(true, true, Phase::MatchOver));
        assert!(
            !near_death_wanted(false, true, Phase::Starting { at_ms: 0 }),
            "PvP: a session downs anyway"
        );
        assert!(!near_death_wanted(
            true,
            false,
            Phase::Fighting { armed: true }
        ));
    }

    #[test]
    fn a_saved_arena_flag_is_cleared_on_the_first_tick_even_when_off_is_wanted() {
        // 2026-11-20: the game closed during an Arena match left 10007520 ON in the save; a BR match was immortal.
        assert!(
            saved_flag_write_due(None, false),
            "first tick: clear what the save holds"
        );
        assert!(saved_flag_write_due(None, true));
        assert!(
            !saved_flag_write_due(Some(false), false),
            "then only changes"
        );
        assert!(!saved_flag_write_due(Some(true), true));
        assert!(saved_flag_write_due(Some(true), false));
        assert!(saved_flag_write_due(Some(false), true));
    }

    fn missing(m: bool) -> Timer {
        Timer {
            rival_missing: m,
            ..Timer::default()
        }
    }

    #[test]
    fn the_next_round_waits_until_local_is_back_on_its_spawn() {
        let mut r = Rounds::default();
        start(&mut r);
        let f = r.fight_since_ms;
        r.step(f + 100, true, DOWN, Some(UP));
        let at = f + 100;
        let unplaced = Timer {
            unplaced: true,
            ..Timer::default()
        };
        // The reset happens; the move is refused (mid-attack): the screen stays black past FADE_IN_MS.
        for t in [FADE_OUT_MS, RESET_AFTER_MS] {
            r.step_timed(at + t, true, UP, Some(UP), unplaced);
        }
        assert!(r
            .step_timed(at + FADE_IN_MS + 200, true, UP, Some(UP), unplaced)
            .is_empty());
        // The forced move is written: the round starts.
        let out = r.step_timed(at + FADE_IN_MS + 300, true, UP, Some(UP), Timer::default());
        assert_eq!(out[0], Do::FadeIn);
        // A move that never lands does not hold the match forever.
        let mut r2 = Rounds::default();
        start(&mut r2);
        let f = r2.fight_since_ms;
        r2.step(f + 100, true, DOWN, Some(UP));
        for t in [FADE_OUT_MS, RESET_AFTER_MS] {
            r2.step_timed(f + 100 + t, true, UP, Some(UP), unplaced);
        }
        let out = r2.step_timed(
            f + 100 + FADE_IN_MS + PLACE_WAIT_MS,
            true,
            UP,
            Some(UP),
            unplaced,
        );
        assert_eq!(out[0], Do::FadeIn);
    }

    #[test]
    fn the_first_fade_in_waits_for_the_bot_on_its_spawn() {
        let mut r = Rounds::default();
        assert!(r.step_timed(0, true, UP, None, missing(true)).is_empty());
        assert!(
            r.step_timed(5_000, true, UP, None, missing(true))
                .is_empty(),
            "no bot yet: still black"
        );
        assert_eq!(
            r.step_timed(5_050, true, UP, Some(UP), missing(false)),
            vec![Do::FadeIn, Do::Message("Round 1: fight!".into())]
        );
        // Never seen: the round starts anyway after the wait.
        let mut r = Rounds::default();
        r.step_timed(0, true, UP, None, missing(true));
        let end = START_FADE_IN_MS + RIVAL_WAIT_MS;
        assert!(r
            .step_timed(end - 1, true, UP, None, missing(true))
            .is_empty());
        assert_eq!(
            r.step_timed(end, true, UP, None, missing(true))[0],
            Do::FadeIn
        );
    }

    #[test]
    fn the_next_round_waits_for_the_new_bot_but_the_ending_does_not() {
        let mut r = Rounds::default();
        let t = start(&mut r);
        // The bot is down (dead): round won; at the reset the bots feature brings a new one.
        r.step_timed(t, true, UP, Some(DOWN), missing(false));
        assert_eq!(
            r.step_timed(t + 1_000, true, UP, Some(DOWN), missing(false)),
            vec![Do::FadeOut]
        );
        assert_eq!(
            r.step_timed(t + 5_000, true, UP, Some(DOWN), missing(false)),
            vec![Do::Reset]
        );
        assert!(
            r.step_timed(t + 6_000, true, UP, None, missing(true))
                .is_empty(),
            "the new bot is not on its spawn yet"
        );
        assert_eq!(
            r.step_timed(t + 7_500, true, UP, Some(UP), missing(false)),
            vec![Do::FadeIn, Do::Message("Round 2: fight!".into())]
        );
        r.step_timed(t + 7_550, true, UP, Some(UP), missing(false));
        assert_eq!(r.phase, Phase::Fighting { armed: true });
        // Round 2 won too: the ending plays without waiting for a bot.
        let t = t + 10_000;
        r.step_timed(t, true, UP, Some(DOWN), missing(false));
        r.step_timed(t + 1_000, true, UP, Some(DOWN), missing(false));
        r.step_timed(t + 5_000, true, UP, Some(DOWN), missing(false));
        assert_eq!(
            r.step_timed(t + 6_000, true, UP, None, missing(true)),
            vec![Do::FadeInFrozen, Do::Win]
        );
    }

    const OT_MSG: &str = "Overtime! HP drains until someone falls";

    #[test]
    fn at_the_limit_overtime_starts_and_drains_until_the_host_sees_a_fall() {
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        let at = |d| t0 + d;
        let hp = Some((0.6, 0.4));
        // The first tick past the one-minute mark only warns.
        assert_eq!(
            r.step_timed(
                at(ROUND_LIMIT_MS - 1),
                true,
                UP,
                Some(UP),
                timed(true, hp, None)
            ),
            vec![Do::Message("One minute left!".into())]
        );
        // The limit: no winner yet, overtime is announced once and drains every tick.
        assert_eq!(
            r.step_timed(
                at(ROUND_LIMIT_MS),
                true,
                UP,
                Some(UP),
                timed(true, hp, None)
            ),
            vec![Do::Message(OT_MSG.into()), Do::Drain]
        );
        assert_eq!(
            r.step_timed(
                at(ROUND_LIMIT_MS + 100),
                true,
                UP,
                Some(UP),
                timed(true, hp, None)
            ),
            vec![Do::Drain]
        );
        assert_eq!((r.won, r.lost, r.bouts), (0, 0, 0));
        // The rival falls to 1 HP: the host wins the round and publishes it.
        let out = r.step_timed(
            at(ROUND_LIMIT_MS + 20_000),
            true,
            UP,
            Some(DOWN),
            timed(true, hp, None),
        );
        assert_eq!(out[0], Do::Verdict(1, Outcome::Won));
        assert!(matches!(&out[1], Do::Message(m) if m.contains("time!") && m.contains("you won")));
        assert_eq!((r.won, r.lost, r.bouts), (1, 0, 1));
    }

    #[test]
    fn nobody_falling_in_overtime_goes_to_the_hp_judgement() {
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        r.step_timed(
            t0 + ROUND_LIMIT_MS,
            true,
            UP,
            Some(UP),
            timed(true, Some((0.3, 0.6)), None),
        );
        let out = r.step_timed(
            t0 + ROUND_LIMIT_MS + OVERTIME_MS,
            true,
            UP,
            Some(UP),
            timed(true, Some((0.3, 0.6)), None),
        );
        assert_eq!(out[0], Do::Verdict(1, Outcome::Lost));
        assert_eq!((r.won, r.lost), (0, 1));
    }

    #[test]
    fn in_overtime_the_other_game_waits_for_the_hosts_verdict_even_at_one_hp() {
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        let hp = Some((0.1, 0.9));
        assert_eq!(
            r.step_timed(
                t0 + ROUND_LIMIT_MS + 100,
                true,
                UP,
                Some(UP),
                timed(false, hp, None)
            ),
            vec![Do::Message(OT_MSG.into()), Do::Drain]
        );
        // Its own player at 1 HP: no lost round without the host.
        assert_eq!(
            r.step_timed(
                t0 + ROUND_LIMIT_MS + 5_000,
                true,
                DOWN,
                Some(UP),
                timed(false, hp, None)
            ),
            vec![Do::Drain]
        );
        assert_eq!((r.won, r.lost), (0, 0));
        // The host's verdict decides.
        r.step_timed(
            t0 + ROUND_LIMIT_MS + 5_100,
            true,
            DOWN,
            Some(UP),
            timed(false, hp, Some(Outcome::Lost)),
        );
        assert_eq!((r.won, r.lost), (0, 1));
        // No verdict at all: it judges by itself after the overtime and the wait.
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        r.step_timed(
            t0 + ROUND_LIMIT_MS + OVERTIME_MS + VERDICT_WAIT_MS,
            true,
            UP,
            Some(UP),
            timed(false, Some((0.9, 0.1)), None),
        );
        assert_eq!((r.won, r.lost), (1, 0));
    }

    #[test]
    fn a_draw_scores_nothing_and_the_round_is_played_again() {
        // Both at 1 HP in the host's view.
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        r.step_timed(
            t0 + ROUND_LIMIT_MS,
            true,
            UP,
            Some(UP),
            timed(true, None, None),
        );
        let out = r.step_timed(
            t0 + ROUND_LIMIT_MS + 30_000,
            true,
            DOWN,
            Some(DOWN),
            timed(true, None, None),
        );
        assert_eq!(out[0], Do::Verdict(1, Outcome::Draw));
        assert_eq!((r.won, r.lost, r.bouts), (0, 0, 1));
        assert!(matches!(&out[1], Do::Message(m) if m.contains("Round 1") && m.contains("draw")));
        // Even HP at the end of the overtime.
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        r.step_timed(
            t0 + ROUND_LIMIT_MS,
            true,
            UP,
            Some(UP),
            timed(true, None, None),
        );
        let out = r.step_timed(
            t0 + ROUND_LIMIT_MS + OVERTIME_MS,
            true,
            UP,
            Some(UP),
            timed(true, Some((0.5, 0.502)), None),
        );
        assert_eq!(out[0], Do::Verdict(1, Outcome::Draw));
        assert_eq!(judge(None), Outcome::Draw);
        assert_eq!(
            Outcome::from_host_wire(Outcome::Won.wire(None), None),
            Some(Outcome::Lost)
        );
    }

    #[test]
    fn the_hosts_verdict_holds_for_its_teammates_and_flips_for_its_rivals() {
        let from = |o: Outcome, host: Option<u8>, me: Option<u8>| {
            Outcome::from_host_wire(o.wire(host), me).unwrap()
        };
        assert_eq!(
            from(Outcome::Won, Some(0), Some(0)),
            Outcome::Won,
            "teammate"
        );
        assert_eq!(from(Outcome::Won, Some(0), Some(1)), Outcome::Lost, "rival");
        assert_eq!(from(Outcome::Lost, Some(1), Some(0)), Outcome::Won);
        assert_eq!(from(Outcome::Draw, Some(1), Some(0)), Outcome::Draw);
        assert_eq!(from(Outcome::Draw, Some(1), Some(1)), Outcome::Draw);
        // Either team unknown: the 1v1 rule (the other game is the rival).
        assert_eq!(from(Outcome::Won, None, Some(0)), Outcome::Lost);
        assert_eq!(from(Outcome::Won, Some(0), None), Outcome::Lost);
        assert_eq!(Outcome::Won.wire(None), 1, "1v1 values unchanged");
        assert_eq!(Outcome::from_host_wire(0x105, Some(0)), None, "bad outcome");
    }

    /// ARENA-5: start a 2v2 round (LOCAL + one teammate vs two rivals); returns the next free time.
    fn start2(r: &mut Rounds) -> u64 {
        for t in [0, 1_000, 1_050] {
            r.step_teams(t, true, UP, &[UP], &[UP, UP], Timer::default());
        }
        assert_eq!(r.phase, Phase::Fighting { armed: true });
        2_000
    }

    #[test]
    fn a_team_round_waits_for_every_player_up_before_it_counts() {
        let mut r = Rounds::default();
        for t in [0, 1_000, 1_050] {
            r.step_teams(t, true, UP, &[Seen::default()], &[UP, UP], Timer::default());
        }
        assert_eq!(
            r.phase,
            Phase::Fighting { armed: false },
            "a teammate not up"
        );
        r.step_teams(1_100, true, UP, &[UP], &[UP, BEATEN], Timer::default());
        assert_eq!(r.phase, Phase::Fighting { armed: false }, "a rival not up");
        r.step_teams(1_150, true, UP, &[UP], &[UP, UP], Timer::default());
        assert_eq!(r.phase, Phase::Fighting { armed: true });
    }

    #[test]
    fn one_beaten_player_watches_the_team_and_the_round_goes_on() {
        let mut r = Rounds::default();
        let t = start2(&mut r);
        let out = r.step_teams(t, true, BEATEN, &[UP], &[UP, UP], Timer::default());
        assert_eq!(out[0], Do::Spectate);
        assert!(r.spectating);
        assert_eq!(
            r.phase,
            Phase::Fighting { armed: true },
            "the round goes on"
        );
        assert!(
            r.step_teams(t + 100, true, BEATEN, &[UP], &[UP, UP], Timer::default())
                .is_empty(),
            "spectates once"
        );
        // A heal on the bench does not bring LOCAL back.
        assert!(r
            .step_teams(t + 200, true, UP, &[UP], &[UP, BEATEN], Timer::default())
            .is_empty());
        // The teammate falls too: the round is lost.
        let out = r.step_teams(
            t + 300,
            true,
            UP,
            &[BEATEN],
            &[UP, BEATEN],
            Timer::default(),
        );
        assert_eq!(out, vec![Do::Message("Round 1: you lost (0-1)".into())]);
        // The next round starts with nobody on the bench.
        for ms in [1_000, 5_000] {
            r.step_teams(t + 300 + ms, true, UP, &[UP], &[UP, UP], Timer::default());
        }
        let out = r.step_teams(t + 6_300, true, UP, &[UP], &[UP, UP], Timer::default());
        assert_eq!(out[0], Do::FadeIn);
        assert!(!r.spectating);
    }

    #[test]
    fn watching_lasts_from_the_beating_to_the_reset() {
        let mut r = Rounds::default();
        let t = start2(&mut r);
        assert!(!spectating_now(&r));
        r.step_teams(t, true, BEATEN, &[UP], &[UP, UP], Timer::default());
        assert!(spectating_now(&r));
        // The teammate falls: the round is over, LOCAL still watches through the fade-out...
        r.step_teams(
            t + 100,
            true,
            BEATEN,
            &[BEATEN],
            &[UP, UP],
            Timer::default(),
        );
        assert!(matches!(r.phase, Phase::RoundOver { .. }));
        assert!(spectating_now(&r));
        r.step_teams(
            t + 1_200,
            true,
            BEATEN,
            &[BEATEN],
            &[UP, UP],
            Timer::default(),
        );
        assert!(spectating_now(&r), "faded out, not reset yet");
        // ...and stops at the reset (back on its spawn, on the floor).
        let out = r.step_teams(t + 5_200, true, UP, &[UP], &[UP, UP], Timer::default());
        assert!(out.contains(&Do::Reset));
        assert!(!spectating_now(&r));
        // The match ending never watches.
        r.phase = Phase::MatchOver;
        r.spectating = true;
        assert!(!spectating_now(&r));
    }

    #[test]
    fn the_body_is_hidden_only_on_the_bench_and_follows_only_once_hidden() {
        assert_eq!(watch_stage(None, true), WatchStage::Parking);
        assert_eq!(watch_stage(Some(0), true), WatchStage::Parked);
        assert_eq!(
            watch_stage(Some(HIDE_AFTER_MS - 1), true),
            WatchStage::Parked
        );
        assert_eq!(watch_stage(Some(HIDE_AFTER_MS), true), WatchStage::Hidden);
        assert_eq!(
            watch_stage(Some(HIDE_AFTER_MS + FOLLOW_AFTER_MS - 1), true),
            WatchStage::Hidden
        );
        assert_eq!(
            watch_stage(Some(HIDE_AFTER_MS + FOLLOW_AFTER_MS), true),
            WatchStage::Following
        );
        assert_eq!(watch_stage(None, false), WatchStage::Parking);
        assert_eq!(
            watch_stage(Some(0), false),
            WatchStage::Following,
            "arena_hide 0"
        );
    }

    #[test]
    fn the_watched_teammate_stays_until_switched_and_wraps() {
        let a = (0x6002_FFFF_FFFF, [1.0, 0.0, 0.0]);
        let b = (0x6003_FFFF_FFFF, [2.0, 0.0, 0.0]);
        let c = (0x6005_FFFF_FFFF, [3.0, 0.0, 0.0]);
        assert_eq!(pick_watch(None, &[], false, false), None);
        assert_eq!(pick_watch(Some(a.0), &[], true, false), None, "nobody left");
        assert_eq!(
            pick_watch(None, &[c, a, b], false, false),
            Some(a),
            "lowest id"
        );
        assert_eq!(
            pick_watch(Some(b.0), &[c, a, b], false, false),
            Some(b),
            "stays"
        );
        assert_eq!(pick_watch(Some(b.0), &[c, a, b], true, false), Some(c));
        assert_eq!(
            pick_watch(Some(c.0), &[c, a, b], true, false),
            Some(a),
            "wraps"
        );
        assert_eq!(
            pick_watch(Some(a.0), &[c, a, b], false, true),
            Some(c),
            "back wraps"
        );
        // The watched one falls: the next candidate (lowest id).
        assert_eq!(pick_watch(Some(b.0), &[c, a], false, false), Some(a));
        // Positions move with the candidate.
        let b2 = (b.0, [9.0, 1.0, 9.0]);
        assert_eq!(pick_watch(Some(b.0), &[a, b2], false, false), Some(b2));
    }

    #[test]
    fn a_team_round_is_won_only_when_every_rival_is_beaten() {
        let mut r = Rounds::default();
        let t = start2(&mut r);
        assert!(r
            .step_teams(t, true, UP, &[UP], &[BEATEN, UP], Timer::default())
            .is_empty());
        let out = r.step_teams(
            t + 100,
            true,
            UP,
            &[BEATEN],
            &[BEATEN, BEATEN],
            Timer::default(),
        );
        assert_eq!(out, vec![Do::Message("Round 1: you won (1-0)".into())]);
        assert!(!r.spectating, "LOCAL standing never spectates");
    }

    #[test]
    fn a_leaver_leaves_a_team_that_plays_on_short_handed() {
        let mut r = Rounds::default();
        let t = start2(&mut r);
        // A rival left (one rival seen): beating the one left wins.
        let out = r.step_teams(t, true, UP, &[UP], &[BEATEN], Timer::default());
        assert_eq!(out, vec![Do::Message("Round 1: you won (1-0)".into())]);
        // The whole other team gone: no round is won by that.
        let mut r = Rounds::default();
        let t = start2(&mut r);
        assert!(r
            .step_teams(t, true, UP, &[UP], &[], Timer::default())
            .is_empty());
        // LOCAL's teammate left: LOCAL beaten alone loses (no bench).
        let out = r.step_teams(t + 100, true, BEATEN, &[], &[UP], Timer::default());
        assert_eq!(out, vec![Do::Message("Round 1: you lost (0-1)".into())]);
    }

    #[test]
    fn a_downed_winner_still_gets_up_after_a_team_round() {
        let mut r = Rounds::default();
        let t = start2(&mut r);
        r.step_teams(t, true, DOWN, &[UP], &[UP, UP], Timer::default());
        let out = r.step_teams(
            t + 100,
            true,
            DOWN,
            &[UP],
            &[BEATEN, BEATEN],
            Timer::default(),
        );
        assert_eq!(out, vec![Do::Message("Round 1: you won (1-0)".into())]);
        r.step_teams(t + 1_100, true, DOWN, &[UP], &[UP, UP], Timer::default());
        let out = r.step_teams(t + 2_100, true, DOWN, &[UP], &[UP, UP], Timer::default());
        assert_eq!(out, vec![Do::Revive]);
    }

    #[test]
    fn overtime_in_teams_ends_when_a_whole_team_is_beaten() {
        let mut r = Rounds::default();
        let t = start2(&mut r);
        let f = r.fight_since_ms;
        let host = timed(true, Some((0.4, 0.6)), None);
        let _ = t;
        let out = r.step_teams(f + ROUND_LIMIT_MS, true, BEATEN, &[UP], &[UP, UP], host);
        assert!(
            out.contains(&Do::Spectate) && out.contains(&Do::Drain),
            "{out:?}"
        );
        let out = r.step_teams(
            f + ROUND_LIMIT_MS + 100,
            true,
            BEATEN,
            &[UP],
            &[BEATEN, BEATEN],
            host,
        );
        assert_eq!(out[0], Do::Verdict(1, Outcome::Won));
    }

    #[test]
    fn a_side_loses_only_when_it_has_players_and_all_are_beaten() {
        assert!(!all_beaten(&[]));
        assert!(!all_beaten(&[BEATEN, UP]));
        assert!(all_beaten(&[BEATEN, DOWN]));
        assert!(!all_up(&[]));
        assert!(all_up(&[UP, UP]));
        assert_eq!(side_hp(&[]), None);
        assert_eq!(side_hp(&[0.5, 1.0]), Some(0.75));
    }

    #[test]
    fn once_beaten_in_a_round_a_player_stays_beaten() {
        let mut ids = Vec::new();
        assert_eq!(sticky(&mut ids, 7, UP), UP);
        assert_eq!(sticky(&mut ids, 7, BEATEN), BEATEN);
        assert_eq!(sticky(&mut ids, 7, UP), BEATEN, "healed on the bench");
        assert_eq!(sticky(&mut ids, 8, UP), UP, "another player");
    }

    #[test]
    fn one_minute_before_the_limit_a_message_warns_once_per_round() {
        let mut r = Rounds::default();
        start(&mut r);
        let f = r.fight_since_ms;
        let warn = Do::Message("One minute left!".into());
        let at = |t| f + t;
        let tick = |r: &mut Rounds, t| r.step_timed(t, true, UP, Some(UP), timed(true, None, None));
        assert!(tick(&mut r, at(ROUND_LIMIT_MS - WARN_LEFT_MS - 1)).is_empty());
        assert_eq!(
            tick(&mut r, at(ROUND_LIMIT_MS - WARN_LEFT_MS)),
            vec![warn.clone()]
        );
        assert!(tick(&mut r, at(ROUND_LIMIT_MS - 1_000)).is_empty());
        // A short test limit gives no warning.
        let mut r = Rounds::default();
        start(&mut r);
        let f = r.fight_since_ms;
        let short = Timer {
            limit_ms: 30_000,
            ..timed(true, None, None)
        };
        assert!(r
            .step_timed(f + 29_000, true, UP, Some(UP), short)
            .is_empty());
    }

    #[test]
    fn a_down_before_the_limit_still_ends_the_round_normally() {
        let mut r = Rounds::default();
        start(&mut r);
        let t0 = r.fight_since_ms;
        let out = r.step_timed(
            t0 + 10_000,
            true,
            UP,
            Some(DOWN),
            timed(true, Some((0.1, 0.9)), None),
        );
        assert!(matches!(&out[0], Do::Message(m) if !m.contains("time!") && m.contains("you won")));
    }
}
