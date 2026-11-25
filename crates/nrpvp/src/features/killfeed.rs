//! KF-1: the killfeed. `Leo downed Bob`, `Ana killed Bob`, on every player's screen as the game's own toasts (`game::gametoast`),
//! with the names the players gave their characters (`PlayerGameData+0x9C`, which every game holds for every player it has loaded).
//!
//! Only the victim's game knows who hit it (`PlayerIns+0x1A0` is valid there only, MEMORY_MAP §7), so that game reports: the rule
//! [`super::br_match::kill_report`] raises a [`KillEvent`], [`report`] publishes it on `net::bus` and hands it to this game's feed; every other
//! game receives the fact. [`Feed`] turns events into lines; it keeps the names, so a player whose character is gone still has one.
//! No line for a mob's or a fall's death: the game's own respawn follows, so nobody is out. Log lines start with `killfeed:`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::{scoreboard, Ctx, Feature, Phase};
use crate::game::world::World;
use crate::game::{gametoast, layout, mem};
use crate::net::bus::{self, topic};

/// SB: a participant missing from the world this long is gone (a player who left, a bot the game removed); the match rules use the same.
const GONE_AFTER: Duration = Duration::from_secs(6);
/// SB: a rival bot dead this long with no kill report was killed by a mob.
const BOT_DEAD_AFTER: Duration = Duration::from_secs(2);
/// SB: the end-of-match summary waits this long in the Roundtable with no menu open (the arrival's fade and banners).
const SUMMARY_SETTLE: Duration = Duration::from_secs(5);
/// SB: the next page of the summary follows the closed dialog after this long.
const PAGE_GAP: Duration = Duration::from_millis(600);
/// SB: the menu word must read a dialog this long to count as open, and 0 this long to count as closed (it flickers while a dialog
/// opens: score-bots-end run 1794072172 skipped a page).
const DIALOG_OPEN_MIN: Duration = Duration::from_millis(500);
const DIALOG_CLOSED_MIN: Duration = Duration::from_millis(300);
/// SB: a page's dialog closed faster than this was not read (a leftover key press closed it): it is shown once more.
const PAGE_MIN_READ: Duration = Duration::from_millis(1_500);
/// SB: a page's dialog not seen open this long after it was asked for is given up (the next page follows).
const PAGE_NEVER_OPENED: Duration = Duration::from_secs(10);
/// SB: the summary is dropped if the player is not in the Roundtable this long after the match.
const SUMMARY_GIVE_UP: Duration = Duration::from_secs(300);
/// Names are read again this often (a player who just loaded in).
const NAME_EVERY: Duration = Duration::from_secs(1);
/// Characters of a name in `PlayerGameData` (the game's own limit, 16 with the terminator).
const NAME_UNITS: usize = 16;
/// A player with no name known yet.
const NO_NAME: &str = "A player";
/// Names on the feed: this game's own team and every other one, as battle royales colour them (the toast's text takes the game's markup,
/// seen in the game 2026-11-03).
const COLOR_MINE: &str = "#7FC8FF";
const COLOR_RIVAL: &str = "#FF6A5A";

/// What happened to a player. Who it is: the session player id's upper half (`0x6002` of `0x6002FFFFFFFF`), the same on every game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KillKind {
    /// A rival's hit put the player in the downed state.
    Downed,
    /// Out of the match by a rival (the last one who hit).
    Killed,
    /// Out of the match by the Night's Tide.
    Tide,
    /// SB: died to a mob or a fall; the game respawns the player (no line on the feed, a death on the scoreboard).
    Died,
}

/// One line of the feed, as the victim's game saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KillEvent {
    pub(crate) kind: KillKind,
    pub(crate) victim: u16,
    /// `None`: no rival is known (the tide, or a hit whose attacker could not be read).
    pub(crate) killer: Option<u16>,
}

/// The short id of a session player id.
pub(crate) fn short_id(id: u64) -> u16 {
    (id >> 32) as u16
}

/// The bus fact of an event (pure). `seq` makes the key unique: a player can be downed many times.
pub(crate) fn encode(ev: KillEvent, seq: u32) -> (u32, u32) {
    let kind = match ev.kind {
        KillKind::Downed => 0,
        KillKind::Killed => 1,
        KillKind::Tide => 2,
        KillKind::Died => 3,
    };
    (
        (u32::from(ev.victim) << 16) | ((seq & 0x3FFF) << 2) | kind,
        ev.killer.map_or(0, u32::from),
    )
}

/// The event of a bus fact (pure). A build before SB decodes kind 3 (`Died`) as `None` and ignores it.
pub(crate) fn decode(key: u32, value: u32) -> Option<KillEvent> {
    let kind = match key & 3 {
        0 => KillKind::Downed,
        1 => KillKind::Killed,
        2 => KillKind::Tide,
        _ => KillKind::Died,
    };
    Some(KillEvent {
        kind,
        victim: (key >> 16) as u16,
        killer: u16::try_from(value).ok().filter(|k| *k != 0),
    })
}

static LOCAL_EVENTS: Mutex<Vec<KillEvent>> = Mutex::new(Vec::new());
/// Never reset: the facts of a match are forgotten when its lobby is left, but one more number costs nothing.
static SEQ: AtomicU32 = AtomicU32::new(0);
static SHOWN: AtomicU32 = AtomicU32::new(0);
static LAST: Mutex<String> = Mutex::new(String::new());

/// The victim's game: tell everyone (main loop).
pub(crate) fn report(ev: KillEvent) {
    let (key, value) = encode(ev, SEQ.fetch_add(1, Ordering::Relaxed));
    let sent = bus::publish(topic::KILL_FEED, key, value);
    tracing::info!(
        "killfeed: reporting {ev:?}{}",
        if sent { "" } else { " (the bus refused it)" }
    );
    LOCAL_EVENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(ev);
}

/// BOT-KF: an event only this game knows (a bot's death: bots live on this game only). No bus fact.
pub(crate) fn local_event(ev: KillEvent) {
    tracing::info!("killfeed: bot event {ev:?}");
    LOCAL_EVENTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(ev);
}

/// BOT-KF: a bot's name on the feed: "Bot 1", "Bot 2"... in spawn order (the game gives the bots no name: their HP bar reads `?NpcName?`).
pub(crate) fn bot_name(handle: u64) -> String {
    format!("Bot {}", (handle & 0x0FFF) + 1)
}

/// The console's status line.
#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub(crate) fn describe() -> String {
    format!(
        "killfeed: {} line(s) shown, last: {}",
        SHOWN.load(Ordering::Relaxed),
        LAST.lock().unwrap_or_else(PoisonError::into_inner)
    )
}

/// The names, teams and who is out; turns events into lines (pure).
#[derive(Debug, Default)]
pub(crate) struct Feed {
    names: BTreeMap<u16, String>,
    teams: BTreeMap<u16, u64>,
    out: BTreeSet<u16>,
    /// LOCAL's short id.
    me: Option<u16>,
    /// FS7-7: pairs seen in the same tick with the same team key (teammates). A team key from the rosters is an index that shifts
    /// when a player leaves (friend session 2026-11-07: LOCAL went from team 1 to team 0, the key of the player who had left), so
    /// keys of different ticks must not be compared.
    mates: BTreeSet<(u16, u16)>,
    /// The players seen so far in this tick, with their key.
    this_tick: Vec<(u16, u64)>,
}

impl Feed {
    /// A new tick of the player list: keys are compared only inside one tick.
    pub(crate) fn begin_tick(&mut self) {
        self.this_tick.clear();
    }

    /// A player of the list this tick.
    pub(crate) fn see(&mut self, id: u16, team: u64, local: bool) {
        self.teams.insert(id, team);
        if local {
            self.me = Some(id);
        }
        for (other, key) in &self.this_tick {
            if *key == team && *other != id {
                self.mates.insert(((*other).min(id), (*other).max(id)));
            }
        }
        self.this_tick.push((id, team));
    }

    /// ARENA-KF: a bot seen alive again under an id that was out (a new bot on a cleared bot's handle) can get a line again.
    pub(crate) fn back(&mut self, id: u16) {
        self.out.remove(&id);
    }

    /// Whether two participants were ever seen as teammates (one is the other included).
    fn mates_with(&self, a: u16, b: u16) -> bool {
        a == b || self.mates.contains(&(a.min(b), a.max(b)))
    }

    /// FS7-7: a participant's team for the summary: the smallest short id among everyone linked to it by [`Feed::mates`].
    fn group_of(&self, id: u16) -> u16 {
        let mut group = BTreeSet::from([id]);
        loop {
            let more: Vec<u16> = self
                .mates
                .iter()
                .filter_map(|(a, b)| match (group.contains(a), group.contains(b)) {
                    (true, false) => Some(*b),
                    (false, true) => Some(*a),
                    _ => None,
                })
                .collect();
            if more.is_empty() {
                return *group.first().unwrap_or(&id);
            }
            group.extend(more);
        }
    }

    /// A player's name as the game holds it (a blank one is no name). The game's texts are markup, a name must not be.
    pub(crate) fn name(&mut self, id: u16, name: &str) {
        let name = gametoast::clean(name.trim());
        if !name.is_empty() {
            self.names.insert(id, name);
        }
    }

    /// Everyone seen this expedition with their name and team (the end-of-match summary).
    pub(crate) fn rows(&self) -> Vec<scoreboard::Row> {
        self.teams
            .keys()
            .map(|id| scoreboard::Row {
                id: *id,
                name: self.plain_name(*id),
                team: u64::from(self.group_of(*id)),
            })
            .collect()
    }

    /// A player's name as plain text (the console's score table).
    pub(crate) fn plain_name(&self, id: u16) -> String {
        self.names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("{id:#06X}"))
    }

    /// A player's name in the colour of their side.
    fn who(&self, id: u16) -> String {
        let name = self.names.get(&id).map_or(NO_NAME, String::as_str);
        let mine = self.me.is_some_and(|me| self.mates_with(me, id));
        let color = if mine { COLOR_MINE } else { COLOR_RIVAL };
        format!("<font color='{color}'>{name}</font>")
    }

    /// The line of an event, and the book kept. A player already out gets no second line (a report that came twice).
    pub(crate) fn line(&mut self, ev: KillEvent) -> Option<String> {
        if ev.kind == KillKind::Died || self.out.contains(&ev.victim) {
            return None;
        }
        let victim = self.who(ev.victim);
        let killer = ev.killer.map(|k| self.who(k));
        if ev.kind != KillKind::Downed {
            self.out.insert(ev.victim);
        }
        Some(match (ev.kind, killer) {
            (KillKind::Downed, Some(k)) => format!("{k} downed {victim}"),
            (KillKind::Downed, None) => format!("{victim} is down"),
            (KillKind::Killed, Some(k)) => format!("{k} killed {victim}"),
            (KillKind::Killed, None) => format!("{victim} was killed"),
            (KillKind::Tide, _) => format!("{victim} was consumed by the tide"),
            (KillKind::Died, _) => return None,
        })
    }
}

/// The name in a `PlayerGameData`, as far as it reads (game memory).
fn read_name(player: usize) -> Option<String> {
    let data = mem::read::<usize>(player + layout::PLAYER_GAMEDATA).filter(|p| mem::is_ptr(*p))?;
    let units = mem::read::<[u16; NAME_UNITS]>(data + layout::GAMEDATA_NAME)?;
    let n = units.iter().position(|u| *u == 0).unwrap_or(NAME_UNITS);
    Some(String::from_utf16_lossy(&units[..n]))
}

/// SB: a summary page whose dialog was asked for: when, whether it was seen open, and the menu word since when.
#[derive(Debug)]
struct PageWait {
    at: Instant,
    /// The page's text (shown again once if its dialog closed too fast to be read), and whether it is already the second time.
    text: String,
    again: bool,
    /// When the dialog was seen open (the menu word read a dialog long enough).
    opened_at: Option<Instant>,
    opened: bool,
    word: Option<u32>,
    word_since: Instant,
}

/// The feature: names from the world, events from the bus and from this game's rules, lines to the screen.
pub(crate) struct KillFeed {
    feed: Feed,
    /// SB: every participant's kills and deaths (fed every event the feed sees).
    tally: scoreboard::Tally,
    active: bool,
    names_at: Option<Instant>,
    /// SB: this expedition was a Battle Royale match (`World::in_match` was seen).
    in_match: bool,
    /// SB: LOCAL's short id this expedition.
    me: Option<u16>,
    /// SB: when each participant was last seen in the world, and whether it is a bot.
    seen: BTreeMap<u16, (Instant, bool)>,
    /// SB: since when each rival bot has read HP 0 (a bot's body stays in the world after its death).
    bot_dead: BTreeMap<u16, Instant>,
    /// SB: the end-of-match summary's dialog pages still to show, since when (the match end), and the rows in all.
    summary: Option<(std::collections::VecDeque<String>, Instant, usize)>,
    /// SB: since when the Roundtable has been calm (no menu, no dialog), and the page whose dialog was asked for: when, and whether
    /// it was seen open (the menu word is not 0 while a dialog is open).
    calm_since: Option<Instant>,
    page_asked: Option<PageWait>,
    pages_shown: usize,
    /// SB: the next page is one shown again.
    again_next: bool,
    /// SB-ARENA: LOCAL's Arena rounds (won, lost) this match and whether the rival is a bot (`None` = not an Arena match).
    arena: Option<(u8, u8, bool)>,
}

impl KillFeed {
    pub(crate) fn new() -> Self {
        KillFeed {
            feed: Feed::default(),
            tally: scoreboard::Tally::default(),
            active: false,
            names_at: None,
            in_match: false,
            me: None,
            seen: BTreeMap::new(),
            bot_dead: BTreeMap::new(),
            summary: None,
            calm_since: None,
            page_asked: None,
            pages_shown: 0,
            again_next: false,
            arena: None,
        }
    }
}

impl Feature for KillFeed {
    fn name(&self) -> &'static str {
        "killfeed"
    }
    fn phase(&self) -> Phase {
        Phase::AfterCombat
    }
    fn tick(&mut self, w: &World, ctx: &mut Ctx) {
        let received = bus::take(topic::KILL_FEED);
        let local =
            std::mem::take(&mut *LOCAL_EVENTS.lock().unwrap_or_else(PoisonError::into_inner));
        if !w.in_expedition() {
            if self.active {
                self.active = false;
                let rows = self.feed.rows();
                if self.in_match && (rows.len() >= 2 || self.arena.is_some()) {
                    let text = match self.arena {
                        Some((won, lost, vs_bot)) => self.arena_summary(&rows, won, lost, vs_bot),
                        None => scoreboard::summary(&self.tally, &rows, self.me),
                    };
                    tracing::info!(
                        "scoreboard: match over, summary of {} rows waits for the Roundtable:\n{text}",
                        rows.len()
                    );
                    let pages = scoreboard::pages(&text).into();
                    self.summary = Some((pages, Instant::now(), rows.len()));
                    self.pages_shown = 0;
                    self.page_asked = None;
                }
                self.feed = Feed::default();
                self.tally = scoreboard::Tally::default();
                // FS7-1: the own-score text is this game's; a stale one must never show in the next match.
                let _ = scoreboard::write_message(
                    w.base,
                    &scoreboard::message(scoreboard::Line::default()),
                );
                self.names_at = None;
                self.in_match = false;
                self.me = None;
                self.seen.clear();
                self.bot_dead.clear();
                self.arena = None;
            }
            self.show_summary(w, ctx);
            return;
        }
        self.summary = None;
        self.active = true;
        self.in_match |= w.in_match;
        if let Some((won, lost)) = arena_score() {
            self.arena = Some((won, lost, crate::mm::arena_bots()));
        }
        let now = Instant::now();
        let refresh = self.names_at.is_none_or(|t| t.elapsed() >= NAME_EVERY);
        if refresh {
            self.names_at = Some(Instant::now());
        }
        self.feed.begin_tick();
        for f in w.fighters.iter().flatten() {
            let id = short_id(f.id);
            self.feed.see(id, f.team, f.local);
            self.seen.insert(id, (now, false));
            if f.local {
                self.me = Some(id);
            }
            if refresh {
                if let Some(name) = read_name(f.addr) {
                    self.feed.name(id, &name);
                }
            }
        }
        // BOT-KF: every rival bot is a rival of its own, named "Bot N". ARENA-KF: an Arena Duo / Trio bot ally is named the same and
        // shares LOCAL's team key (blue, LOCAL's team in the summary).
        let my_key = w
            .fighters
            .iter()
            .flatten()
            .find(|f| f.local)
            .map(|f| f.team);
        let allies = crate::features::bots::arena_allies();
        let bots = w
            .rival_bots()
            .map(|b| (b, None))
            .chain(allies.iter().filter_map(|h| w.bot(*h)).map(|b| (b, my_key)));
        for (b, key) in bots {
            let sid = crate::game::world::bot_session_id(b.handle);
            let id = short_id(sid);
            self.feed.see(id, key.unwrap_or(sid), false);
            self.feed.name(id, &bot_name(b.handle));
            self.seen.insert(id, (now, true));
            if b.hp == 0 {
                self.bot_dead.entry(id).or_insert(now);
            } else {
                self.bot_dead.remove(&id);
                // ARENA-KF: the game hands a cleared bot's handle to a new bot (the Arena refill); the new one can die again.
                self.feed.back(id);
            }
        }
        let events = local.into_iter().chain(
            received
                .iter()
                .filter_map(|r| decode(r.fact.key, r.fact.value)),
        );
        let mut changed = false;
        for ev in events {
            self.tally.record(ev);
            changed = true;
            if let Some(line) = self.feed.line(ev) {
                tracing::info!("killfeed: {line}");
                gametoast::show(&line);
                SHOWN.fetch_add(1, Ordering::Relaxed);
                *LAST.lock().unwrap_or_else(PoisonError::into_inner) = line;
            }
        }
        // SB: a participant gone from the world: a player who left, or a bot a mob killed (the game replaces it under a new name).
        // A dead bot with no kill report after BOT_DEAD_AFTER was killed by a mob (a rival's kill reports within a tick or two).
        let mut gone: Vec<(u16, bool)> = self
            .seen
            .iter()
            .filter(|(_, (at, _))| now.duration_since(*at) >= GONE_AFTER)
            .map(|(id, (_, bot))| (*id, *bot))
            .collect();
        gone.extend(
            self.bot_dead
                .iter()
                .filter(|(id, at)| {
                    now.duration_since(**at) >= BOT_DEAD_AFTER && !self.tally.get(**id).out
                })
                .map(|(id, _)| (*id, true)),
        );
        for (id, bot) in gone {
            self.seen.remove(&id);
            if !self.tally.get(id).out {
                tracing::info!(
                    "scoreboard: {} is gone ({})",
                    self.feed.plain_name(id),
                    if bot {
                        "a bot replaced after a mob's kill"
                    } else {
                        "left the match"
                    }
                );
            }
            self.tally.gone(id, bot);
            changed = true;
        }
        if changed || refresh {
            scoreboard::set_table(self.tally.table(|id| self.feed.plain_name(id)));
        }
        // SB: opening the pause menu (or the console) shows LOCAL's own numbers as the game's centre message.
        if w.pause_opened | scoreboard::take_show_asked() {
            let me = w
                .fighters
                .iter()
                .flatten()
                .find(|f| f.local)
                .map(|f| short_id(f.id));
            if let Some(me) = me {
                let text = match self.arena {
                    Some((won, lost, _)) => scoreboard::arena_message(won, lost),
                    None => scoreboard::message(self.tally.get(me)),
                };
                match scoreboard::write_message(w.base, &text) {
                    Ok(()) => {
                        ctx.flags.push((scoreboard::FLAG_SCORE, true));
                        tracing::info!("scoreboard: own score shown: {text}");
                    }
                    Err(e) => tracing::warn!("scoreboard: own score not shown: {e}"),
                }
            }
        }
    }
}

/// SB-ARENA: LOCAL's Arena rounds while an Arena match is on.
fn arena_score() -> Option<(u8, u8)> {
    crate::game::arenarounds::score()
}

impl KillFeed {
    /// SB-ARENA: the end of an Arena match: the round score, LOCAL against its one rival (a bot reads "Bot": a bot killed in a round
    /// is replaced by a new one under a new name).
    fn arena_summary(&self, rows: &[scoreboard::Row], won: u8, lost: u8, vs_bot: bool) -> String {
        let me = self
            .me
            .map_or_else(|| NO_NAME.to_string(), |id| self.feed.plain_name(id));
        let rival = rows.iter().find(|r| Some(r.id) != self.me);
        let rival_name = if vs_bot {
            "Bot".to_string()
        } else {
            rival.map_or_else(|| "Rival".to_string(), |r| r.name.clone())
        };
        let left = !vs_bot && rival.is_some_and(|r| self.tally.has_left(r.id));
        scoreboard::arena_summary(&me, &rival_name, won, lost, left)
    }

    /// SB: the end-of-match summary as the game's dialog, once LOCAL stands in the Roundtable with no menu open for a moment.
    /// The dialog shows three lines, so the summary comes in pages: each one after the previous dialog was closed (OK).
    fn show_summary(&mut self, w: &World, ctx: &mut Ctx) {
        let Some((pages, since, rows)) = &mut self.summary else {
            return;
        };
        if since.elapsed() >= SUMMARY_GIVE_UP {
            tracing::info!("scoreboard: summary dropped (no Roundtable after the match)");
            self.summary = None;
            return;
        }
        let word = crate::game::menugate::menu_word(w.base);
        // The page asked for: wait until its dialog was open for a while and is closed again (or it never opened).
        if let Some(p) = &mut self.page_asked {
            if word != p.word {
                tracing::info!(
                    "scoreboard: page {}: menu word {:?} -> {:?}",
                    self.pages_shown,
                    p.word,
                    word
                );
                p.word = word;
                p.word_since = Instant::now();
            }
            let steady = p.word_since.elapsed();
            if word.is_some_and(|m| m != 0) && steady >= DIALOG_OPEN_MIN && !p.opened {
                p.opened = true;
                p.opened_at = Some(p.word_since);
            }
            let closed = p.opened && word == Some(0) && steady >= DIALOG_CLOSED_MIN;
            if !closed && (p.opened || p.at.elapsed() < PAGE_NEVER_OPENED) {
                return;
            }
            if !p.opened {
                tracing::warn!("scoreboard: page {} was not seen open", self.pages_shown);
            }
            // Closed by a key press left over from the result screens: show the page once more.
            let read_for = p
                .opened_at
                .map_or(Duration::ZERO, |t| p.word_since.duration_since(t));
            if p.opened && read_for < PAGE_MIN_READ && !p.again {
                tracing::info!(
                    "scoreboard: page {} closed after {} ms: shown again",
                    self.pages_shown,
                    read_for.as_millis()
                );
                pages.push_front(std::mem::take(&mut p.text));
                self.pages_shown -= 1;
                self.again_next = true;
            }
            self.page_asked = None;
            self.calm_since = None;
        }
        if pages.is_empty() {
            tracing::info!(
                "scoreboard: summary shown ({rows} rows, {} pages)",
                self.pages_shown
            );
            self.summary = None;
            return;
        }
        let calm = w.place == Some(crate::game::views::Place::Roundtable)
            && w.local.is_some()
            && word == Some(0);
        if !calm {
            self.calm_since = None;
            return;
        }
        let settle = if self.pages_shown == 0 {
            SUMMARY_SETTLE
        } else {
            PAGE_GAP
        };
        if self.calm_since.get_or_insert_with(Instant::now).elapsed() < settle {
            return;
        }
        let total = self.pages_shown + pages.len();
        let page = pages.pop_front().unwrap_or_default();
        let again = std::mem::take(&mut self.again_next);
        match scoreboard::write_summary(w.base, &page) {
            Ok(()) => {
                ctx.flags.push((scoreboard::FLAG_SUMMARY, true));
                self.pages_shown += 1;
                self.page_asked = Some(PageWait {
                    at: Instant::now(),
                    text: page,
                    again,
                    opened_at: None,
                    opened: false,
                    word,
                    word_since: Instant::now(),
                });
                tracing::info!(
                    "scoreboard: summary page {}/{total} shown",
                    self.pages_shown
                );
            }
            Err(e) => {
                tracing::warn!("scoreboard: summary not shown: {e}");
                self.summary = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u16 = 0x6001;
    const B: u16 = 0x6002;
    const C: u16 = 0x6003;

    /// A is LOCAL, three teams of one.
    fn feed() -> Feed {
        let mut f = Feed::default();
        for (id, name) in [(A, "Leo"), (B, "Bob"), (C, "Ana")] {
            f.see(id, u64::from(id), id == A);
            f.name(id, name);
        }
        f
    }

    fn ev(kind: KillKind, victim: u16, killer: Option<u16>) -> KillEvent {
        KillEvent {
            kind,
            victim,
            killer,
        }
    }

    /// The line as the player reads it.
    fn plain(line: Option<String>) -> String {
        let line = line.expect("a line");
        line.replace(&format!("<font color='{COLOR_MINE}'>"), "")
            .replace(&format!("<font color='{COLOR_RIVAL}'>"), "")
            .replace("</font>", "")
    }

    #[test]
    fn a_fact_carries_the_event_and_every_report_has_its_own_key() {
        for e in [
            ev(KillKind::Downed, B, Some(A)),
            ev(KillKind::Killed, B, Some(C)),
            ev(KillKind::Killed, B, None),
            ev(KillKind::Tide, C, None),
        ] {
            let (key, value) = encode(e, 7);
            assert_eq!(decode(key, value), Some(e));
            assert_ne!(encode(e, 8).0, key);
        }
        let (key, value) = encode(ev(KillKind::Died, B, None), 9);
        assert_eq!(key & 3, 3, "the kind a build before SB ignores");
        assert_eq!(decode(key, value), Some(ev(KillKind::Died, B, None)));
    }

    #[test]
    fn bots_have_names_and_their_kills_read_like_players() {
        let mut f = feed();
        let bot = short_id(crate::game::world::bot_session_id(0xFFFF_FFFF_10E0_0002));
        assert_eq!(bot, 0xB002);
        f.see(bot, 0xB002, false);
        f.name(bot, &bot_name(0xFFFF_FFFF_10E0_0002));
        assert_eq!(
            plain(f.line(ev(KillKind::Killed, bot, Some(A)))),
            "Leo killed Bot 3"
        );
        let other = short_id(crate::game::world::bot_session_id(0xFFFF_FFFF_10E0_0004));
        f.see(other, 0xB004, false);
        f.name(other, &bot_name(0xFFFF_FFFF_10E0_0004));
        assert_eq!(
            plain(f.line(ev(KillKind::Killed, A, Some(other)))),
            "Bot 5 killed Leo"
        );
        assert_eq!(
            plain(f.line(ev(KillKind::Tide, other, None))),
            "Bot 5 was consumed by the tide"
        );
    }

    #[test]
    fn the_short_id_is_the_upper_half() {
        assert_eq!(short_id(0x6002_FFFF_FFFF), 0x6002);
    }

    #[test]
    fn downed_then_killed_reads_as_in_a_battle_royale() {
        let mut f = feed();
        assert_eq!(
            plain(f.line(ev(KillKind::Downed, B, Some(A)))),
            "Leo downed Bob"
        );
        // A third player finishes: the kill is theirs.
        assert_eq!(
            plain(f.line(ev(KillKind::Killed, B, Some(C)))),
            "Ana killed Bob"
        );
    }

    #[test]
    fn my_team_is_blue_and_everyone_else_is_red() {
        let mut f = feed();
        assert_eq!(
            f.line(ev(KillKind::Downed, B, Some(A))).unwrap(),
            format!(
                "<font color='{COLOR_MINE}'>Leo</font> downed <font color='{COLOR_RIVAL}'>Bob</font>"
            )
        );
    }

    #[test]
    fn the_longest_line_fits_a_placeholder() {
        let mut f = Feed::default();
        for (id, team) in [(A, 1), (B, 2), (C, 3)] {
            f.see(id, team, id == A);
            f.name(id, "WWWWWWWWWWWWWWW");
        }
        let line = f.line(ev(KillKind::Killed, B, Some(A))).unwrap();
        assert!(
            line.encode_utf16().count() < gametoast::TEXT_UNITS,
            "{line}"
        );
    }

    #[test]
    fn a_mob_death_has_no_line_and_does_not_put_the_player_out() {
        let mut f = feed();
        assert_eq!(f.line(ev(KillKind::Died, B, None)), None);
        assert!(f.line(ev(KillKind::Killed, B, Some(A))).is_some());
    }

    #[test]
    fn the_tide_has_its_own_line() {
        let mut f = feed();
        assert_eq!(
            plain(f.line(ev(KillKind::Tide, C, None))),
            "Ana was consumed by the tide"
        );
    }

    /// FS7-7, friend session 2026-11-07 (M2): three Solo players; when C leaves, the roster keys shift and LOCAL takes C's old key.
    /// The summary must still keep everyone apart, and nobody becomes blue.
    #[test]
    fn shifted_team_keys_after_a_leave_do_not_make_teammates() {
        let mut f = Feed::default();
        let key = |t: u64| (1 << 63) | t;
        f.begin_tick();
        for (id, team) in [(C, 0), (A, 1), (B, 2)] {
            f.see(id, key(team), id == A);
            f.name(id, "x");
        }
        // C left: A and B are renumbered 0 and 1.
        f.begin_tick();
        f.see(A, key(0), true);
        f.see(B, key(1), false);
        let rows = f.rows();
        let teams: BTreeSet<u64> = rows.iter().map(|r| r.team).collect();
        assert_eq!(teams.len(), 3, "{rows:?}");
        assert!(f
            .line(ev(KillKind::Killed, C, Some(B)))
            .unwrap()
            .contains(&format!(
                "<font color='{COLOR_RIVAL}'>x</font> killed <font color='{COLOR_RIVAL}'>x"
            )));
    }

    /// ARENA-KF: a new bot on a cleared bot's handle (the Arena refill) gets its own line; the same death reported twice does not.
    #[test]
    fn a_bot_back_on_a_reused_handle_can_get_a_line_again() {
        let mut f = Feed::default();
        for (id, team) in [(A, 1), (B, 1), (C, 2)] {
            f.see(id, team, id == A);
            f.name(id, "x");
        }
        assert!(f.line(ev(KillKind::Killed, C, Some(B))).is_some());
        assert!(
            f.line(ev(KillKind::Killed, C, Some(B))).is_none(),
            "a report twice"
        );
        f.back(C);
        let line = f.line(ev(KillKind::Killed, C, Some(A))).unwrap();
        assert!(
            line.contains(&format!("<font color='{COLOR_MINE}'>x</font> killed")),
            "{line}"
        );
    }

    #[test]
    fn a_teammate_is_blue_too() {
        let mut f = Feed::default();
        for (id, team) in [(A, 1), (B, 1), (C, 2)] {
            f.see(id, team, id == A);
            f.name(id, "x");
        }
        assert_eq!(
            f.line(ev(KillKind::Killed, B, Some(C))).unwrap(),
            format!(
                "<font color='{COLOR_RIVAL}'>x</font> killed <font color='{COLOR_MINE}'>x</font>"
            )
        );
    }

    #[test]
    fn a_player_with_no_name_or_no_killer_still_has_a_line() {
        let mut f = Feed::default();
        f.name(A, "   ");
        assert_eq!(
            plain(f.line(ev(KillKind::Downed, A, Some(B)))),
            "A player downed A player"
        );
        assert_eq!(
            plain(f.line(ev(KillKind::Killed, A, None))),
            "A player was killed"
        );
    }

    #[test]
    fn a_player_who_is_out_gets_no_second_line_and_a_revive_lets_them_go_down_again() {
        let mut f = feed();
        assert!(f.line(ev(KillKind::Downed, B, Some(A))).is_some());
        assert!(f.line(ev(KillKind::Downed, B, Some(C))).is_some());
        assert!(f.line(ev(KillKind::Killed, B, Some(C))).is_some());
        assert!(f.line(ev(KillKind::Killed, B, Some(C))).is_none());
    }

    #[test]
    fn a_name_cannot_carry_markup() {
        let mut f = feed();
        f.name(A, "<font>x");
        assert_eq!(
            plain(f.line(ev(KillKind::Downed, B, Some(A)))),
            " font x downed Bob"
        );
    }
}
