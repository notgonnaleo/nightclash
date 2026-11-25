//! UI-1 option 1 (`br_menu = true`): Battle Royale matchmaking from the stock expedition menu. Commence Expedition starts our queue
//! (with the Number of Players option as the mode) and the game's own dialog says so; Commence again stops it; once the match is
//! ready, Commence works as normal so the host starts the expedition.
//!
//! No code byte is written (game 1.03.3, exe 1.3.3.0; UI-1 card in TASKS.md):
//! - **Commence** (hardware execute breakpoint, `crate::hwbp` slot 2) at `exe+0x9EEBFB`, the first instruction after the prologue of the
//!   Commence check sequence `0x9EEBE0` (`r14` = the menu; settings `[r14+0xFC8]`; Number of Players byte `+0x49`: 2 one player,
//!   1 two players, 0 three players, from the Commence label code `0x9EF3F0`). To show a message the handler sets
//!   `RIP = exe+0x9EEC1D`, the game's own branch that shows an OK dialog with CL_Dialogues text 506040 and returns normally (proven by
//!   the C1 prototype, `c1proto.rs`).
//! - **Text**: entry 506040's string-offset slot in the CL_Dialogues FMG points at one of our UTF-16 texts (`fmg + offset`, see
//!   `c1proto.rs`). The handler picks the text by storing `text - fmg` into that slot just before the dialog is built. 506040 ("not
//!   fully installed") is not otherwise shown on an installed game.
//! - **Labels** (hardware execute breakpoint, `crate::hwbp` slot 3) at `exe+0x7AC800`, the CL_MenuText getter (`edx` = text id,
//!   category 200 with its `_dlc01` twin 504 asked first). Seamless Co-op's `LookupEntry` hook replaces 130409 / 130417 / 130408 (One /
//!   Two / Three players) with "Solo Mission" / "Up to 6 players"; the handler swaps those ids for ours (99100100..=99100102 "Solo" /
//!   "Duo" / "Trio", CL_MenuText_dlc01 entries from the game-data package, `scripts/build-gamedata.ps1`) before the lookup runs, so
//!   SC's hook never sees them. Armed only once our first id is found in the live CL_MenuText_dlc01 FMG (no package = SC's labels).
//! - OFF-1: the game mode (`crate::features::mode`) is read here too. In Offline, Commence with Solo is left to the game (its own
//!   solo expedition, no queue); Duo/Trio show a dialog that Offline is solo only.
//! - The handler never takes a lock: it records a request in an atomic; this module's thread hands it to the matchmaking worker
//!   (`crate::mm::send`), the same path as the overlay's queue buttons.
//!
//! Log lines start with `br_menu:`.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::STATUS_SINGLE_STEP;
use windows::Win32::System::Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS};

use super::fmg::{dialogues_fmg, offset_slot};
use super::gamemem::RealMem;
use super::views::ExpeditionMenu;
use crate::dr::{EFLAGS_RF, EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};
use crate::features::mode::GameMode;
use crate::mm::{PHASE_MATCHED, PHASE_SEARCHING};

// R1: moved here from the dev-only C1 prototype (`c1proto`), which now imports them.
/// First instruction after the prologue of the Commence checks: `mov rcx,[rip+3A3FCA6h]` (game 1.03.3).
pub(crate) const RVA_AFTER_PROLOGUE: usize = 0x9E_EBFB;
pub(crate) const AFTER_PROLOGUE_BYTES: [u8; 7] = [0x48, 0x8B, 0x0D, 0xA6, 0xFC, 0xA3, 0x03];
/// The 506040 notice-dialog branch: `lea rax,[rbp+50h]` … `mov edx,7B8B8h` (game 1.03.3).
pub(crate) const RVA_NOTICE_BRANCH: usize = 0x9E_EC1D;
pub(crate) const NOTICE_BRANCH_BYTES: [u8; 13] = [
    0x48, 0x8D, 0x45, 0x50, 0x48, 0x89, 0x45, 0x58, 0xBA, 0xB8, 0xB8, 0x07, 0x00,
];
pub(crate) const NOTICE_ID: u32 = 506_040;

/// The CL_MenuText getter `getter(out, id)` (game 1.03.3): `mov [rsp+8],rcx; push rbx; sub rsp,30h`.
pub(crate) const RVA_MENUTEXT_GETTER: usize = 0x7A_C800;
pub(crate) const MENUTEXT_GETTER_BYTES: [u8; 10] =
    [0x48, 0x89, 0x4C, 0x24, 0x08, 0x53, 0x48, 0x83, 0xEC, 0x30];
/// CL_MenuText_dlc01 (FMG file id in menu_dlc01.msgbnd).
const CAT_MENUTEXT_DLC: u32 = 504;
/// Our label ids (gamedata/fmg-entries.json).
pub(crate) const LABEL_SOLO: u32 = 99_100_100;
pub(crate) const LABEL_DUO: u32 = 99_100_101;
pub(crate) const LABEL_TRIO: u32 = 99_100_102;
/// BOT-5: the "No. of Password Players" row as **Bots**: its label and its two values ("Two players" = On, "Three players" = Off).
pub(crate) const LABEL_BOTS: u32 = 99_100_105;
pub(crate) const LABEL_BOTS_ON: u32 = 99_100_106;
pub(crate) const LABEL_BOTS_OFF: u32 = 99_100_107;
/// The row's own predicate `exe+0x9F91C0` (true = greyed out: no multiplayer password, or Number of Players not three; the only callers are
/// the two variants of this row, `exe+0x9F78A4` / `exe+0x9F7E04`). Its first instruction is a `jmp`, so returning at the entry is safe.
pub(crate) const RVA_BOTS_ROW_PREDICATE: usize = 0x9F_91C0;
pub(crate) const BOTS_ROW_PREDICATE_BYTES: [u8; 5] = [0xE9, 0x82, 0xD5, 0x92, 0x06];
/// BOT-5: the Bots row is on in both builds (Leonardo, 2026-11-07, after his second bots match: players may train against bots; it was
/// dev-only for a few hours). `false` would leave the row as the game's own and never start a match against bots.
pub(crate) const BOTS_MENU: bool = true;
/// The main loop says whether LOCAL is in an expedition (the Bots row's breakpoint is armed only in the Roundtable: in an expedition slot 0
/// belongs to the death hold and the revive block).
static IN_EXPEDITION: AtomicBool = AtomicBool::new(false);
/// Called by the main loop every tick.
pub(crate) fn set_in_expedition(on: bool) {
    IN_EXPEDITION.store(on, Ordering::Relaxed);
}

/// `CSMenuMan+0x48` with the expedition menu open (nro's `MENU_EXPEDITION`, 2026-10-27).
const MENU_WORD_EXPEDITION: u32 = 0x20_0004;
/// The CL_LineHelp FMG's entries of the row (help line; value Two; value Three) and the text that identifies the FMG.
const HELP_IDS: [u32; 3] = [701_002, 701_011, 701_012];
const HELP_PROBE: (u32, &str) = (701_010, "Set number of players for expedition");
/// "Stop Searching": the Commence button while our queue is searching.
pub(crate) const LABEL_STOP: u32 = 99_100_103;
/// UI-3: the Commence button's label while the Battle Royale card is selected and no queue is running.
pub(crate) const LABEL_FIND: u32 = 99_100_104;
/// The Commence button's labels (`0x9EF3F0`): 130300 / 130302 "Commence Expedition", 130304 "Commence Solo Expedition", 130305
/// "Commence Two-Player Expedition" (130301 / 130303 "Cancel" belong to the game's own standby and are left alone).
pub(crate) const COMMENCE_LABELS: [u32; 4] = [130_300, 130_302, 130_304, 130_305];

/// The id the menu should ask for instead of the game's (pure; unit tested): our Solo / Duo / Trio for Number of Players, and "Stop
/// Searching" for the Commence button while the queue is searching.
pub(crate) fn label_swap(id: u32, phase: u8) -> Option<u32> {
    match id {
        130_409 => Some(LABEL_SOLO),
        130_417 => Some(LABEL_DUO),
        130_408 => Some(LABEL_TRIO),
        130_002 => Some(LABEL_BOTS),
        130_200 => Some(LABEL_BOTS_ON),
        130_201 => Some(LABEL_BOTS_OFF),
        _ if phase == PHASE_SEARCHING && COMMENCE_LABELS.contains(&id) => Some(LABEL_STOP),
        _ => None,
    }
}

/// UI-3: [`label_swap`] plus the Battle Royale card's own button label. `br_card` is only asked for a Commence label (it reads the menu).
pub(crate) fn label_for_card(
    id: u32,
    phase: u8,
    br_card: impl FnOnce() -> Option<bool>,
) -> Option<u32> {
    label_swap(id, phase).or_else(|| {
        (COMMENCE_LABELS.contains(&id) && br_card() == Some(true)).then_some(LABEL_FIND)
    })
}

/// Number of Players byte values (`0x9EF3F0`: 2 → 130304 "Commence Solo Expedition", 1 → 130305 "Two-Player", else three).
pub(crate) const PLAYERS_ONE: u8 = 2;
pub(crate) const PLAYERS_TWO: u8 = 1;
pub(crate) const PLAYERS_THREE: u8 = 0;

/// FS7-4 (pure): the Number of Players byte for a party of `n` (Solo / Duo / Trio).
pub(crate) fn players_for_party(n: usize) -> u8 {
    match n {
        0 | 1 => PLAYERS_ONE,
        2 => PLAYERS_TWO,
        _ => PLAYERS_THREE,
    }
}

/// FS7-3 / FS7-4: the expedition menu just opened (`menu` = its object, freshly seen by the label hook), and again when its Matchmaking
/// Settings tab first shows in this visit: opening that tab puts the saved values back over the menu's (seen 2026-11-08, run
/// 1794100721: Trio and Bots On over our Solo and Off). In the first visit of a launch the Bots switch goes Off (the game saves it,
/// Leonardo wants every launch to start without bots); every time, while the queue is idle, Number of Players is set from the own
/// party's size. A choice the player makes after that is kept.
fn preset_menu(menu: usize, bots_off: bool, when: &str) {
    let mem = RealMem;
    let Some(settings) = ExpeditionMenu::new(&mem, menu).settings() else {
        return;
    };
    if bots_off && BOTS_MENU {
        if settings.bots() == Some(true) {
            match settings.set_bots(false) {
                Ok(()) => tracing::info!("br_menu: Bots switch reset to Off ({when})"),
                Err(e) => tracing::warn!("br_menu: Bots switch not reset: {e}"),
            }
        } else {
            tracing::info!("br_menu: Bots switch reset to Off ({when}; it was Off)");
        }
    }
    if crate::mm::phase() != crate::mm::PHASE_IDLE {
        return;
    }
    let Some(n) = crate::mm::party_size() else {
        return;
    };
    let want = players_for_party(n);
    if settings.players() != Some(want) {
        match settings.set_players(want) {
            Ok(()) => tracing::info!(
                "br_menu: Number of Players set to {} (party of {n}; {when})",
                match want {
                    PLAYERS_ONE => "Solo",
                    PLAYERS_TWO => "Duo",
                    _ => "Trio",
                }
            ),
            Err(e) => tracing::warn!("br_menu: Number of Players not set: {e}"),
        }
    }
}

/// Our dialog texts (index into [`TEXTS`]).
pub(crate) const T_SEARCHING: usize = 0;
pub(crate) const T_STOPPED: usize = 1;
pub(crate) const T_SEARCHING_DUO: usize = 2;
pub(crate) const T_OFF: usize = 3;
pub(crate) const T_OFFLINE_SOLO: usize = 4;
pub(crate) const T_SEARCHING_TRIO: usize = 5;
pub(crate) const T_STARTING: usize = 6;
/// BOT-5: Bots On with Duo / Trio.
pub(crate) const T_BOTS_SOLO: usize = 7;
/// BOT-5: the row's help texts (CL_LineHelp 701002 / 701011 / 701012 point at these).
pub(crate) const T_HELP_ROW: usize = 8;
pub(crate) const T_HELP_ON: usize = 9;
pub(crate) const T_HELP_OFF: usize = 10;
/// ARENA-1: Commence on the Arena card (Solo) starts the Arena search.
pub(crate) const T_ARENA_SEARCHING: usize = 11;
/// ARENA-5: Commence on the Arena card with Duo / Trio starts the Arena search for 2 vs 2 / 3 vs 3.
pub(crate) const T_ARENA_DUO: usize = 12;
pub(crate) const T_ARENA_TRIO: usize = 13;
const N_TEXTS: usize = 14;
const TEXT_UNITS: usize = 256;

/// Requests from the handler to the thread.
pub(crate) const REQ_NONE: u8 = 0;
pub(crate) const REQ_QUEUE_SOLO: u8 = 1;
pub(crate) const REQ_LEAVE: u8 = 2;
/// Duo / Trio (the matchmaker's `Mode::Duo` / `Mode::Squad`): the Seamless Co-op party queues, led by the owner of its lobby.
pub(crate) const REQ_QUEUE_DUO: u8 = 3;
pub(crate) const REQ_QUEUE_TRIO: u8 = 4;
/// ARENA-1: an Arena queue, Solo (1v1).
pub(crate) const REQ_QUEUE_ARENA_SOLO: u8 = 5;
/// ARENA-5: an Arena queue, Duo (2v2) / Trio (3v3); a short party is filled with random players as in Battle Royale (MM-FILL).
pub(crate) const REQ_QUEUE_ARENA_DUO: u8 = 6;
pub(crate) const REQ_QUEUE_ARENA_TRIO: u8 = 7;

/// What a Commence press does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// Let the game run Commence unchanged.
    Pass,
    /// Match ready: set the Number of Players byte to `PLAYERS_TWO` (a multiplayer start), then let Commence run.
    PassMultiplayer,
    /// BOT-5: Bots On, Solo: the game's own solo Commence, and the expedition is a match against bots.
    PassBots,
    /// ARENA-4: Bots On, the Arena card: the game's own solo Commence, and the expedition is an Arena match against bots. ARENA-6: the
    /// team size (1 Solo: one bot; 2 Duo: LOCAL + 1 bot ally vs 2 bots; 3 Trio: LOCAL + 2 bot allies vs 3 bots).
    PassArenaBots(u8),
    /// Show our dialog `text` instead of Commence and hand `request` to the thread.
    Dialog { text: usize, request: u8 },
}

/// UI-3: the decision for a Commence press given the selected card. Only the Battle Royale card uses our queue; any other card is the
/// game's (and Seamless Co-op's) own expedition, untouched. While our queue is searching or a match is ready the card does not matter
/// (a player who changed card still stops the search or starts the match). `br_card` is `None` when the menu cannot be read: left to the
/// game.
pub(crate) fn decide_for_card(
    br_card: Option<bool>,
    players: u8,
    phase: u8,
    mm_on: bool,
    mode: GameMode,
    bots: bool,
) -> Decision {
    let queue_active = phase == PHASE_SEARCHING || phase == PHASE_MATCHED;
    if br_card != Some(true) && !queue_active {
        return Decision::Pass;
    }
    // BOT-5: Bots On (an idle queue, the Battle Royale mode): no search, a match against bots at once. Solo only for now (BOTS.md §2f).
    if bots && !queue_active && mode == GameMode::BattleRoyale {
        return if players == PLAYERS_ONE {
            Decision::PassBots
        } else {
            Decision::Dialog {
                text: T_BOTS_SOLO,
                request: REQ_NONE,
            }
        };
    }
    decide(players, phase, mm_on, mode)
}

/// ARENA-1 (pure): a Commence on the Arena card with the queue idle starts the Arena search (ARENA-5: Solo, Duo or Trio); `None` = not the
/// Arena card, not idle (Stop Searching and a found match go through [`decide_for_card`] as for Battle Royale) or Offline (the game's
/// own solo expedition, as any other card). ARENA-4: with Bots On, a match against bots at once (no search, no queue needed); ARENA-6: Duo
/// and Trio too (LOCAL with bot allies against a bot team), as the game's own solo expedition.
pub(crate) fn decide_arena(
    arena_card: Option<bool>,
    players: u8,
    phase: u8,
    mm_on: bool,
    mode: GameMode,
    bots: bool,
) -> Option<Decision> {
    if arena_card != Some(true) || phase != crate::mm::PHASE_IDLE || mode != GameMode::BattleRoyale
    {
        return None;
    }
    Some(if bots {
        Decision::PassArenaBots(match players {
            PLAYERS_ONE => 1,
            PLAYERS_TWO => 2,
            _ => 3,
        })
    } else if !mm_on {
        Decision::Dialog {
            text: T_OFF,
            request: REQ_NONE,
        }
    } else if players == PLAYERS_ONE {
        Decision::Dialog {
            text: T_ARENA_SEARCHING,
            request: REQ_QUEUE_ARENA_SOLO,
        }
    } else if players == PLAYERS_TWO {
        Decision::Dialog {
            text: T_ARENA_DUO,
            request: REQ_QUEUE_ARENA_DUO,
        }
    } else {
        Decision::Dialog {
            text: T_ARENA_TRIO,
            request: REQ_QUEUE_ARENA_TRIO,
        }
    })
}

/// MM-3g + MM-7: once a match is found it starts by itself, so a Commence pressed in the moment before the player is in the host's lobby must
/// not run the game's own expedition (pure). After that the menu is in its party state, which never reaches this hook.
pub(crate) fn while_starting(d: Decision, phase: u8, auto_start: bool) -> Decision {
    if auto_start && phase == PHASE_MATCHED {
        Decision::Dialog {
            text: T_STARTING,
            request: REQ_NONE,
        }
    } else {
        d
    }
}

/// The decision for a Commence press (pure; unit tested).
pub(crate) fn decide(players: u8, phase: u8, mm_on: bool, mode: GameMode) -> Decision {
    // OFF-1: Offline runs the game's own solo Commence whatever the queue is doing (switching to Offline leaves the queue).
    if mode == GameMode::Offline {
        return if players == PLAYERS_ONE {
            Decision::Pass
        } else {
            Decision::Dialog {
                text: T_OFFLINE_SOLO,
                request: REQ_NONE,
            }
        };
    }
    if !mm_on {
        return Decision::Dialog {
            text: T_OFF,
            request: REQ_NONE,
        };
    }
    match phase {
        PHASE_MATCHED => {
            if players == PLAYERS_ONE {
                Decision::PassMultiplayer
            } else {
                Decision::Pass
            }
        }
        PHASE_SEARCHING => Decision::Dialog {
            text: T_STOPPED,
            request: REQ_LEAVE,
        },
        _ => match players {
            PLAYERS_ONE => Decision::Dialog {
                text: T_SEARCHING,
                request: REQ_QUEUE_SOLO,
            },
            PLAYERS_TWO => Decision::Dialog {
                text: T_SEARCHING_DUO,
                request: REQ_QUEUE_DUO,
            },
            _ => Decision::Dialog {
                text: T_SEARCHING_TRIO,
                request: REQ_QUEUE_TRIO,
            },
        },
    }
}

pub(crate) fn texts() -> [String; N_TEXTS] {
    [
        "Battle Royale: searching for players (Solo).\nYou will be told when the match is found.\nTo stop, press Stop Searching.".to_string(),
        "Stopped searching for players.".to_string(),
        "Battle Royale: searching for players (Duo).\nYour Seamless Co-op party queues together, led by its host.\nTo stop, press Stop Searching.".to_string(),
        "Matchmaking is turned off (matchmaking = false).".to_string(),
        "Offline mode is solo only.
Choose Solo in Matchmaking Settings > Number of Players.".to_string(),
        "Battle Royale: searching for players (Trio).\nYour Seamless Co-op party queues together, led by its host.\nTo stop, press Stop Searching.".to_string(),
        "Match found.\nThe match starts by itself.".to_string(),
        "Matches with bots are Solo only for now.\nChoose Solo in Number of Players, or set Bots to Off.".to_string(),
        "Bots (beta): an offline training mode. On: you alone against 5 AI Nightfarers, no other players, starts at once.".to_string(),
        "Bots On: Commence starts an offline training match, Solo against 5 AI bots (no other players).".to_string(),
        "Bots Off: Commence searches for players (the online Battle Royale).".to_string(),
        "Arena: searching for an opponent (Solo, 1 vs 1).
The match starts by itself in the Nightlord's arena.
To stop, press Stop Searching.".to_string(),
        "Arena: searching for players (Duo, 2 vs 2).
Your party queues together; a missing teammate is found for you.
To stop, press Stop Searching.".to_string(),
        "Arena: searching for players (Trio, 3 vs 3).
Your party queues together; missing teammates are found for you.
To stop, press Stop Searching.".to_string(),
    ]
}

struct Texts(UnsafeCell<[[u16; TEXT_UNITS]; N_TEXTS]>);
// SAFETY: written once by this module's thread before the handler can use them (publication through `READY`, release/acquire);
// read-only afterwards (the game reads them as strings).
unsafe impl Sync for Texts {}
static TEXTS: Texts = Texts(UnsafeCell::new([[0; TEXT_UNITS]; N_TEXTS]));

static ADDR: AtomicUsize = AtomicUsize::new(0);
/// BOT-5: the Bots row predicate while its breakpoint is armed (0 = not armed).
static PRED_ADDR: AtomicUsize = AtomicUsize::new(0);
static PRED_HITS: AtomicU32 = AtomicU32::new(0);
static LABEL_ADDR: AtomicUsize = AtomicUsize::new(0);
static LABEL_SWAPS: AtomicU32 = AtomicU32::new(0);
/// The menu the Commence button last asked its text for (console `br_card`).
static LAST_MENU: AtomicUsize = AtomicUsize::new(0);
static TARGET: AtomicUsize = AtomicUsize::new(0);
/// The CL_Dialogues FMG and the address of entry 506040's offset slot (0 = not resolved: Commence is left alone).
static FMG: AtomicUsize = AtomicUsize::new(0);
static SLOT: AtomicUsize = AtomicUsize::new(0);
static REQUEST: AtomicU8 = AtomicU8::new(REQ_NONE);
static PRESSES: AtomicU32 = AtomicU32::new(0);
/// ARENA-3: Commence presses the game itself runs (any `Pass*` decision): the expedition start follows, and it wipes the Sparring
/// loadout while the player still stands at the Roundtable (MEMORY_MAP §29).
static GAME_COMMENCES: AtomicU32 = AtomicU32::new(0);
/// Bug #2 (Arena test 2026-11-11: a Commence started a vanilla solo run and no press was counted): every hit of the Commence
/// breakpoint, and the hits that left the game alone because the menu's settings or the FMG slot could not be read (bit 0 = no
/// settings, 1 = no players byte, 2 = no FMG / slot), so the pipe thread can tell "never hit" from "hit and passed through".
static COMMENCE_HITS: AtomicU32 = AtomicU32::new(0);
static COMMENCE_SKIPS: AtomicU32 = AtomicU32::new(0);
static LAST_SKIP_WHY: AtomicU8 = AtomicU8::new(0);

/// ARENA-3: how many Commence presses the game has run so far.
#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub(crate) fn game_commences() -> u32 {
    GAME_COMMENCES.load(Ordering::Relaxed)
}
/// Last press: players byte, phase and decision code (for the log).
static LAST: AtomicU64 = AtomicU64::new(0);

fn text_addr(i: usize) -> usize {
    // SAFETY: index < N_TEXTS; only the address is taken.
    unsafe { (*TEXTS.0.get())[i].as_ptr() as usize }
}

fn decision_code(d: Decision) -> u64 {
    match d {
        Decision::Pass => 1,
        Decision::PassMultiplayer => 2,
        Decision::PassBots => 3,
        Decision::PassArenaBots(n) => 4 | (u64::from(n) << 4),
        Decision::Dialog { text, request } => 0x100 | ((text as u64) << 4) | u64::from(request),
    }
}

unsafe extern "system" fn veh(info: *mut EXCEPTION_POINTERS) -> i32 {
    // SAFETY: Windows passes a valid EXCEPTION_POINTERS; pointers are null-checked. The reads go through the menu object the game is
    // using in this very function (r14 was just set from rcx) and its settings object, which the next instructions read too; the one
    // write to the settings byte is the same byte the game's own Number of Players list writes; the FMG slot address was validated
    // (committed, writable, 8-aligned) by the thread.
    unsafe {
        if info.is_null() {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        let rec = (*info).ExceptionRecord;
        let ctx = (*info).ContextRecord;
        if rec.is_null() || ctx.is_null() || (*rec).ExceptionCode != STATUS_SINGLE_STEP {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        let rip = (*ctx).Rip as usize;
        if rip != 0 && rip == PRED_ADDR.load(Ordering::Relaxed) {
            // BOT-5: the Bots row is never greyed out: return false at the entry (a `jmp`, nothing pushed yet): al = 0, pop the return
            // address.
            let rsp = (*ctx).Rsp as usize;
            let ret = *(rsp as *const u64);
            (*ctx).Rax &= !0xFF;
            (*ctx).Rip = ret;
            (*ctx).Rsp = (rsp + 8) as u64;
            PRED_HITS.fetch_add(1, Ordering::Relaxed);
            (*ctx).EFlags |= EFLAGS_RF;
            return EXCEPTION_CONTINUE_EXECUTION;
        }
        if rip != 0 && rip == LABEL_ADDR.load(Ordering::Relaxed) {
            // `rbx` still holds the expedition menu at the call that asks for the Commence button's text (`exe+0x9EF45E`, 1.03.3).
            let menu = (*ctx).Rbx as usize;
            let id = (*ctx).Rdx as u32;
            let ours = label_for_card(id, crate::mm::phase(), || {
                LAST_MENU.store(menu, Ordering::Relaxed);
                ExpeditionMenu::new(&RealMem, menu).is_battle_royale_card()
            });
            let bots_label = matches!(ours, Some(LABEL_BOTS | LABEL_BOTS_ON | LABEL_BOTS_OFF));
            if let Some(ours) = ours.filter(|_| BOTS_MENU || !bots_label) {
                (*ctx).Rdx = u64::from(ours);
                LABEL_SWAPS.fetch_add(1, Ordering::Relaxed);
            }
            (*ctx).EFlags |= EFLAGS_RF;
            return EXCEPTION_CONTINUE_EXECUTION;
        }
        if rip == 0 || rip != ADDR.load(Ordering::Relaxed) {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        // R2: the menu and its settings through the typed view (guarded reads, gated write; no lock, no VirtualQuery on reads).
        let mem = RealMem;
        let settings = ExpeditionMenu::new(&mem, (*ctx).R14 as usize).settings();
        let players = settings.as_ref().and_then(|s| s.players());
        let br_card = ExpeditionMenu::new(&mem, (*ctx).R14 as usize).is_battle_royale_card();
        let bots = BOTS_MENU && settings.as_ref().and_then(|s| s.bots()).unwrap_or(false);
        let (fmg, slot) = (FMG.load(Ordering::Acquire), SLOT.load(Ordering::Acquire));
        COMMENCE_HITS.fetch_add(1, Ordering::Relaxed);
        let why = u8::from(settings.is_none())
            | (u8::from(players.is_none()) << 1)
            | (u8::from(fmg == 0 || slot == 0) << 2);
        if why != 0 {
            LAST_SKIP_WHY.store(why, Ordering::Relaxed);
            COMMENCE_SKIPS.fetch_add(1, Ordering::Relaxed);
        }
        if let (Some(settings), Some(players), true) = (settings, players, fmg != 0 && slot != 0) {
            let phase = crate::mm::phase();
            let mode = crate::features::mode::current();
            let arena_card = ExpeditionMenu::new(&mem, (*ctx).R14 as usize)
                .has_card(crate::game::layout::ARENA_CARD_ROW);
            let d = while_starting(
                decide_arena(arena_card, players, phase, crate::mm::enabled(), mode, bots)
                    .unwrap_or_else(|| {
                        decide_for_card(br_card, players, phase, crate::mm::enabled(), mode, bots)
                    }),
                phase,
                crate::mm::auto_start(),
            );
            PRESSES.fetch_add(1, Ordering::Relaxed);
            LAST.store(
                (u64::from(mode == GameMode::Offline) << 40)
                    | (u64::from(players) << 32)
                    | (u64::from(phase) << 24)
                    | decision_code(d),
                Ordering::Relaxed,
            );
            if !matches!(d, Decision::Dialog { .. }) {
                GAME_COMMENCES.fetch_add(1, Ordering::Relaxed);
            }
            match d {
                Decision::Pass => {}
                Decision::PassBots => crate::features::bots::set_bots_match(true),
                // ARENA-6: the team size is ours; the game must run its own SOLO Commence (Duo / Trio would start its matchmaking), so
                // the players byte goes to one first, as PassMultiplayer writes two.
                Decision::PassArenaBots(n) => {
                    if n > 1 {
                        let _ = settings.set_players(PLAYERS_ONE);
                    }
                    crate::mm::set_arena_bots(n);
                }
                Decision::PassMultiplayer => {
                    let _ = settings.set_players(PLAYERS_TWO);
                }
                Decision::Dialog { text, request } => {
                    *(slot as *mut u64) = (text_addr(text) as u64).wrapping_sub(fmg as u64);
                    if request != REQ_NONE {
                        REQUEST.store(request, Ordering::Release);
                    }
                    (*ctx).Rip = TARGET.load(Ordering::Relaxed) as u64;
                }
            }
        }
        (*ctx).EFlags |= EFLAGS_RF;
    }
    EXCEPTION_CONTINUE_EXECUTION
}

/// BOT-5: point the Bots row's CL_LineHelp entries at our texts. The FMG is the category whose entry [`HELP_PROBE`] reads the vanilla
/// English text (the game's category ids are not documented for CL_LineHelp; 200..=215 are tried). `Some(count)` once done.
fn point_help_texts(base: usize) -> Option<usize> {
    let fmg = (200..=215).find_map(|cat| {
        let f = super::fmg::fmg_for(base, cat)?;
        let at = super::fmg::text_at(f, HELP_PROBE.0)?;
        let want: Vec<u16> = HELP_PROBE.1.encode_utf16().collect();
        let got = super::mem::read::<[u16; 36]>(at)?;
        (got[..want.len()] == want[..]).then_some(f)
    })?;
    let mut n = 0;
    for (id, text) in HELP_IDS
        .into_iter()
        .zip([T_HELP_ROW, T_HELP_ON, T_HELP_OFF])
    {
        let Some((slot, _)) = offset_slot(fmg, id) else {
            continue;
        };
        if slot % 8 != 0 || super::mem::read::<u64>(slot).is_none() {
            continue;
        }
        let off = (text_addr(text) as u64).wrapping_sub(fmg as u64);
        if super::mem::write_bytes(slot, &off.to_le_bytes()).is_ok() {
            n += 1;
        }
    }
    (n > 0).then_some(n)
}

/// BOT-5: console `bots_switch [on|off] [solo]`: read or set the Bots switch (`solo` also sets Number of Players to Solo) of the expedition menu last seen (the menu must have been open once;
/// the game keeps the value in its matchmaking settings, so it is remembered between sessions).
#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub(crate) fn bots_switch_cmd(arg: Option<&str>, solo: bool) -> Result<String, String> {
    let menu = LAST_MENU.load(Ordering::Relaxed);
    if menu == 0 {
        return Err("no expedition menu seen yet (open it at the table once)".into());
    }
    let mem = RealMem;
    let settings = ExpeditionMenu::new(&mem, menu)
        .settings()
        .ok_or("menu settings unreadable")?;
    if let Some(a) = arg {
        let on = match a {
            "on" | "1" => true,
            "off" | "0" => false,
            _ => return Err("usage: bots_switch [on|off]".into()),
        };
        settings.set_bots(on).map_err(str::to_string)?;
        if solo {
            settings.set_players(PLAYERS_ONE).map_err(str::to_string)?;
        }
        tracing::info!(
            "br_menu: Bots switch set {} (console)",
            if on { "On" } else { "Off" }
        );
    }
    Ok(format!(
        "bots switch: {}",
        match settings.bots() {
            Some(true) => "On",
            Some(false) => "Off",
            None => "unreadable",
        }
    ))
}

/// Whether the card selected right now in the expedition menu is the Battle Royale card (`None`: no menu known). Console `br_card`.
#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub(crate) fn last_card_is_br() -> Option<bool> {
    match LAST_MENU.load(Ordering::Relaxed) {
        0 => None,
        menu => ExpeditionMenu::new(&RealMem, menu).is_battle_royale_card(),
    }
}

/// ARENA-1: whether the card selected right now is the Arena card (`None`: no menu known). Console `br_card`.
#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub(crate) fn last_card_is_arena() -> Option<bool> {
    match LAST_MENU.load(Ordering::Relaxed) {
        0 => None,
        menu => ExpeditionMenu::new(&RealMem, menu).has_card(crate::game::layout::ARENA_CARD_ROW),
    }
}

/// The Number of Players byte of the expedition menu (`None`: no menu known). Console `br_card`.
#[cfg_attr(not(feature = "dev"), allow(dead_code))]
pub(crate) fn last_players() -> Option<u8> {
    match LAST_MENU.load(Ordering::Relaxed) {
        0 => None,
        menu => ExpeditionMenu::new(&RealMem, menu).settings()?.players(),
    }
}

/// Start the thread (`br_menu = true` with the queue: developer mode with `mm_queue`, or the player build with `matchmaking`).
pub(crate) fn spawn(base: usize) {
    let r = std::thread::Builder::new()
        .name("nrpvp-brmenu".into())
        .spawn(move || {
            if std::panic::catch_unwind(|| run(base)).is_err() {
                crate::hwbp::clear(crate::hwbp::SLOT_COMMENCE);
                tracing::error!("br_menu: thread panicked; Commence is back to normal");
            }
        });
    if r.is_err() {
        tracing::warn!("br_menu: could not spawn the thread");
    }
}

fn run(base: usize) {
    let check = |rva: usize, want: &[u8]| {
        super::mem::read::<[u8; 13]>(base + rva).is_some_and(|b| b[..want.len()] == *want)
    };
    if !check(RVA_AFTER_PROLOGUE, &AFTER_PROLOGUE_BYTES)
        || !check(RVA_NOTICE_BRANCH, &NOTICE_BRANCH_BYTES)
    {
        tracing::warn!("br_menu: code bytes differ from game 1.03.3; Commence left alone");
        return;
    }
    let all = texts();
    for (i, t) in all.iter().enumerate() {
        let units: Vec<u16> = t.encode_utf16().take(TEXT_UNITS - 1).collect();
        // SAFETY: nothing reads TEXTS before FMG/SLOT are published below (Release), and only this thread writes it.
        let dst = unsafe { &mut (*TEXTS.0.get())[i] };
        dst[..units.len()].copy_from_slice(&units);
        dst[units.len()] = 0;
    }
    ADDR.store(base + RVA_AFTER_PROLOGUE, Ordering::Relaxed);
    TARGET.store(base + RVA_NOTICE_BRANCH, Ordering::Relaxed);
    // SAFETY: `veh` has the PVECTORED_EXCEPTION_HANDLER signature; it returns CONTINUE_SEARCH for anything not on our address.
    if unsafe { AddVectoredExceptionHandler(1, Some(veh)) }.is_null() {
        tracing::warn!("br_menu: AddVectoredExceptionHandler failed; Commence left alone");
        return;
    }
    crate::hwbp::set(crate::hwbp::SLOT_COMMENCE, base + RVA_AFTER_PROLOGUE);
    tracing::info!(
        "br_menu: Commence handler armed (hwbp slot 2); waiting for the CL_Dialogues FMG"
    );
    let labels_ok = check(RVA_MENUTEXT_GETTER, &MENUTEXT_GETTER_BYTES);
    if !labels_ok {
        tracing::warn!("br_menu: menu text getter bytes differ from game 1.03.3; Number of Players labels stay as they are");
    }
    let mut labels_armed = false;
    let pred_ok = check(RVA_BOTS_ROW_PREDICATE, &BOTS_ROW_PREDICATE_BYTES);
    if !pred_ok {
        tracing::warn!("br_menu: Bots row predicate bytes differ from game 1.03.3; the Bots row stays greyed out");
    }
    let mut pred_armed = false;
    let mut help_done = false;
    let mut seen_pred = 0u32;
    let mut seen_swaps = 0u32;
    let mut next_resolve = Instant::now();
    let mut seen = 0u32;
    let mut seen_skips = 0u32;
    // FS7-3 / FS7-4: the menu was open on the last pass; a preset waits for the label hook to see the newly opened menu.
    let mut menu_was_open = false;
    let mut preset_pending = false;
    let mut first_menu = true;
    // This visit's menu object, whether this visit resets Bots, and the Matchmaking Settings tab: the Bots row predicate's call count
    // at the open (a higher count = the tab is drawn), when it was first seen and how many re-presets were done since.
    let mut visit_menu = 0usize;
    let mut visit_bots_off = false;
    let mut tab_hits = 0u32;
    let mut tab_seen: Option<Instant> = None;
    let mut tab_done = 0usize;
    const TAB_PRESET_AFTER_MS: [u64; 2] = [200, 800];
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let now = Instant::now();
        let menu_open = !IN_EXPEDITION.load(Ordering::Relaxed)
            && super::menugate::menu_word(base) == Some(MENU_WORD_EXPEDITION);
        if menu_open && !menu_was_open {
            LAST_MENU.store(0, Ordering::Relaxed);
            preset_pending = true;
            visit_menu = 0;
            tab_hits = PRED_HITS.load(Ordering::Relaxed);
            tab_seen = None;
            tab_done = 0;
        }
        menu_was_open = menu_open;
        if preset_pending && menu_open {
            let m = LAST_MENU.load(Ordering::Relaxed);
            if m != 0 {
                preset_pending = false;
                preset_menu(m, first_menu, "menu opened");
                visit_menu = m;
                visit_bots_off = first_menu;
                first_menu = false;
            }
        }
        if menu_open && visit_menu != 0 && tab_done < TAB_PRESET_AFTER_MS.len() {
            if tab_seen.is_none() && PRED_HITS.load(Ordering::Relaxed) > tab_hits {
                tab_seen = Some(now);
            }
            if tab_seen.is_some_and(|t| {
                t.elapsed() >= Duration::from_millis(TAB_PRESET_AFTER_MS[tab_done])
            }) {
                preset_menu(visit_menu, visit_bots_off, "settings tab shown");
                tab_done += 1;
            }
        }
        let resolve_tick = now >= next_resolve;
        if resolve_tick {
            next_resolve = now + Duration::from_secs(2);
            match dialogues_fmg(base).and_then(|f| offset_slot(f, NOTICE_ID).map(|(s, _)| (f, s))) {
                Some((f, s)) if f != FMG.load(Ordering::Relaxed) => {
                    if s % 8 == 0 && super::mem::read::<u64>(s).is_some() {
                        SLOT.store(s, Ordering::Release);
                        FMG.store(f, Ordering::Release);
                        tracing::info!("br_menu: CL_Dialogues FMG 0x{f:X}, entry {NOTICE_ID} slot 0x{s:X}: Commence is now Battle Royale");
                    }
                }
                Some(_) => {}
                None => {
                    if FMG.swap(0, Ordering::AcqRel) != 0 {
                        SLOT.store(0, Ordering::Release);
                        tracing::info!(
                            "br_menu: CL_Dialogues FMG gone; Commence left alone until it is back"
                        );
                    }
                }
            }
        }
        // Labels: arm once our package's ids are in the live CL_MenuText_dlc01 (else the list would show nothing).
        if labels_ok && !labels_armed && resolve_tick {
            if let Some(f) = super::fmg::fmg_for(base, CAT_MENUTEXT_DLC) {
                if [
                    LABEL_SOLO,
                    LABEL_DUO,
                    LABEL_TRIO,
                    LABEL_STOP,
                    LABEL_FIND,
                    LABEL_BOTS,
                    LABEL_BOTS_ON,
                    LABEL_BOTS_OFF,
                ]
                .iter()
                .all(|id| offset_slot(f, *id).is_some())
                {
                    LABEL_ADDR.store(base + RVA_MENUTEXT_GETTER, Ordering::Relaxed);
                    crate::hwbp::set(crate::hwbp::SLOT_LABELS, base + RVA_MENUTEXT_GETTER);
                    labels_armed = true;
                    tracing::info!("br_menu: our labels found in CL_MenuText_dlc01 0x{f:X}: Number of Players shows Solo / Duo / Trio, No. of Password Players shows Bots Off / On (hwbp slot 3)");
                }
            }
        }
        // BOT-5: the Bots row's breakpoint (hwbp slot 0, shared with the quiet invite) only while the expedition menu is open and our queue
        // is idle; the help texts once the CL_LineHelp FMG is found.
        let want_pred = BOTS_MENU
            && pred_ok
            && labels_armed
            && !IN_EXPEDITION.load(Ordering::Relaxed)
            && crate::mm::phase() == crate::mm::PHASE_IDLE
            && super::menugate::menu_word(base) == Some(MENU_WORD_EXPEDITION);
        if want_pred && !pred_armed && crate::hwbp::is_free(crate::hwbp::SLOT_BOTS_ROW) {
            PRED_ADDR.store(base + RVA_BOTS_ROW_PREDICATE, Ordering::Relaxed);
            crate::hwbp::set(crate::hwbp::SLOT_BOTS_ROW, base + RVA_BOTS_ROW_PREDICATE);
            pred_armed = true;
        } else if !want_pred && pred_armed {
            crate::hwbp::clear(crate::hwbp::SLOT_BOTS_ROW);
            pred_armed = false;
            // The address stays known to the handler a while longer (a late hit must still be ours, TRAPS).
            let hits = PRED_HITS.load(Ordering::Relaxed);
            if hits != seen_pred {
                tracing::info!(
                    "br_menu: Bots row kept selectable ({hits} predicate call(s) so far)"
                );
                seen_pred = hits;
            }
        }
        if BOTS_MENU && labels_armed && !help_done && resolve_tick {
            if let Some(n) = point_help_texts(base) {
                help_done = true;
                tracing::info!("br_menu: Bots row help texts in place ({n} entries)");
            }
        }
        let sw = LABEL_SWAPS.load(Ordering::Relaxed);
        if sw != seen_swaps && (seen_swaps == 0 || sw / 100 != seen_swaps / 100) {
            tracing::info!("br_menu: Number of Players labels swapped {sw} time(s) so far");
            seen_swaps = sw;
        }
        match REQUEST.swap(REQ_NONE, Ordering::AcqRel) {
            REQ_QUEUE_SOLO => {
                let ok = crate::mm::send(crate::mm::Cmd::Queue(crate::mm::Mode::Solo));
                tracing::info!(
                    "br_menu: Commence -> queue Solo ({})",
                    if ok {
                        "sent"
                    } else {
                        "NOT sent: queue off or full"
                    }
                );
            }
            r @ (REQ_QUEUE_DUO | REQ_QUEUE_TRIO) => {
                let mode = if r == REQ_QUEUE_DUO {
                    crate::mm::Mode::Duo
                } else {
                    crate::mm::Mode::Squad
                };
                let ok = crate::mm::send(crate::mm::Cmd::Queue(mode));
                tracing::info!(
                    "br_menu: Commence -> queue {} ({})",
                    mode.label(),
                    if ok {
                        "sent"
                    } else {
                        "NOT sent: queue off or full"
                    }
                );
            }
            r @ (REQ_QUEUE_ARENA_SOLO | REQ_QUEUE_ARENA_DUO | REQ_QUEUE_ARENA_TRIO) => {
                let mode = match r {
                    REQ_QUEUE_ARENA_DUO => crate::mm::Mode::Duo,
                    REQ_QUEUE_ARENA_TRIO => crate::mm::Mode::Squad,
                    _ => crate::mm::Mode::Solo,
                };
                let ok = crate::mm::send(crate::mm::Cmd::QueueArena(mode));
                tracing::info!(
                    "br_menu: Commence on the Arena card -> queue Arena {} ({})",
                    mode.label(),
                    if ok {
                        "sent"
                    } else {
                        "NOT sent: queue off or full"
                    }
                );
            }
            REQ_LEAVE => {
                let ok = crate::mm::send(crate::mm::Cmd::Leave);
                tracing::info!(
                    "br_menu: Commence -> stop searching ({})",
                    if ok { "sent" } else { "NOT sent" }
                );
            }
            _ => {}
        }
        let p = PRESSES.load(Ordering::Relaxed);
        if p != seen {
            let l = LAST.load(Ordering::Relaxed);
            tracing::info!(
                "br_menu: Commence pressed (#{p}): mode {}, Number of Players byte {}, queue phase {}, decision code 0x{:X}",
                if (l >> 40) & 1 == 1 { "offline" } else { "br" },
                (l >> 32) & 0xFF,
                (l >> 24) & 0xFF,
                l & 0xFF_FFFF
            );
            seen = p;
        }
        let skips = COMMENCE_SKIPS.load(Ordering::Relaxed);
        if skips != seen_skips {
            tracing::warn!(
                "br_menu: Commence left to the game untouched ({skips} time(s), {} hit(s) in all): {}",
                COMMENCE_HITS.load(Ordering::Relaxed),
                skip_reason(LAST_SKIP_WHY.load(Ordering::Relaxed))
            );
            seen_skips = skips;
        }
    }
}

/// Bug #2 (pure): the reason bits of a Commence the handler could not decide.
fn skip_reason(why: u8) -> String {
    let mut r = Vec::new();
    if why & 1 != 0 {
        r.push("no menu settings");
    }
    if why & 2 != 0 {
        r.push("no Number of Players byte");
    }
    if why & 4 != 0 {
        r.push("no CL_Dialogues FMG slot");
    }
    r.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mm::PHASE_IDLE;

    #[test]
    fn the_party_size_picks_solo_duo_or_trio() {
        assert_eq!(players_for_party(0), PLAYERS_ONE);
        assert_eq!(players_for_party(1), PLAYERS_ONE);
        assert_eq!(players_for_party(2), PLAYERS_TWO);
        assert_eq!(players_for_party(3), PLAYERS_THREE);
        assert_eq!(players_for_party(5), PLAYERS_THREE);
    }

    #[test]
    fn idle_queues_the_mode_of_the_number_of_players() {
        let idle = |p| decide(p, PHASE_IDLE, true, GameMode::BattleRoyale);
        assert_eq!(
            idle(PLAYERS_ONE),
            Decision::Dialog {
                text: T_SEARCHING,
                request: REQ_QUEUE_SOLO
            }
        );
        assert_eq!(
            idle(PLAYERS_TWO),
            Decision::Dialog {
                text: T_SEARCHING_DUO,
                request: REQ_QUEUE_DUO
            }
        );
        assert_eq!(
            idle(PLAYERS_THREE),
            Decision::Dialog {
                text: T_SEARCHING_TRIO,
                request: REQ_QUEUE_TRIO
            }
        );
    }

    #[test]
    fn searching_commence_stops() {
        for p in [PLAYERS_ONE, PLAYERS_TWO, PLAYERS_THREE] {
            assert_eq!(
                decide(p, PHASE_SEARCHING, true, GameMode::BattleRoyale),
                Decision::Dialog {
                    text: T_STOPPED,
                    request: REQ_LEAVE
                }
            );
        }
    }

    #[test]
    fn matched_commence_runs_as_multiplayer() {
        assert_eq!(
            decide(PLAYERS_ONE, PHASE_MATCHED, true, GameMode::BattleRoyale),
            Decision::PassMultiplayer
        );
        assert_eq!(
            decide(PLAYERS_TWO, PHASE_MATCHED, true, GameMode::BattleRoyale),
            Decision::Pass
        );
    }

    #[test]
    fn with_the_automatic_start_a_found_match_needs_no_commence() {
        let pass = Decision::PassMultiplayer;
        let starting = Decision::Dialog {
            text: T_STARTING,
            request: REQ_NONE,
        };
        assert_eq!(while_starting(pass, PHASE_MATCHED, true), starting);
        assert_eq!(while_starting(pass, PHASE_MATCHED, false), pass);
        assert_eq!(while_starting(pass, PHASE_IDLE, true), pass);
        assert_eq!(while_starting(pass, PHASE_SEARCHING, true), pass);
    }

    #[test]
    fn matchmaking_off_says_so() {
        assert_eq!(
            decide(PLAYERS_ONE, PHASE_IDLE, false, GameMode::BattleRoyale),
            Decision::Dialog {
                text: T_OFF,
                request: REQ_NONE
            }
        );
    }

    #[test]
    fn labels_swap_only_the_number_of_players_and_bots_ids() {
        for ph in [PHASE_IDLE, PHASE_SEARCHING, PHASE_MATCHED] {
            assert_eq!(label_swap(130_409, ph), Some(LABEL_SOLO));
            assert_eq!(label_swap(130_417, ph), Some(LABEL_DUO));
            assert_eq!(label_swap(130_408, ph), Some(LABEL_TRIO));
            assert_eq!(label_swap(130_406, ph), None);
            assert_eq!(label_swap(130_407, ph), None);
        }
    }

    #[test]
    fn commence_reads_stop_searching_only_while_searching() {
        for id in COMMENCE_LABELS {
            assert_eq!(label_swap(id, PHASE_SEARCHING), Some(LABEL_STOP));
            assert_eq!(label_swap(id, PHASE_IDLE), None);
            assert_eq!(label_swap(id, PHASE_MATCHED), None);
        }
        assert_eq!(label_swap(130_301, PHASE_SEARCHING), None);
    }

    #[test]
    fn offline_solo_runs_the_games_own_commence_in_every_phase() {
        for ph in [PHASE_IDLE, PHASE_SEARCHING, PHASE_MATCHED] {
            for mm_on in [true, false] {
                assert_eq!(
                    decide(PLAYERS_ONE, ph, mm_on, GameMode::Offline),
                    Decision::Pass
                );
            }
        }
    }

    #[test]
    fn offline_duo_trio_say_solo_only_and_never_queue() {
        for p in [PLAYERS_TWO, PLAYERS_THREE] {
            for ph in [PHASE_IDLE, PHASE_SEARCHING, PHASE_MATCHED] {
                assert_eq!(
                    decide(p, ph, true, GameMode::Offline),
                    Decision::Dialog {
                        text: T_OFFLINE_SOLO,
                        request: REQ_NONE
                    }
                );
            }
        }
    }

    #[test]
    fn texts_fit_the_buffers() {
        for t in texts() {
            assert!(t.encode_utf16().count() < TEXT_UNITS);
        }
    }

    /// UI-3: the card row (`gamedata/nightbossmenu-rows.json`, written into regulation.bin) names texts that the game-data package must hold.
    #[test]
    fn the_battle_royale_card_gets_its_own_button_label() {
        for id in COMMENCE_LABELS {
            assert_eq!(
                label_for_card(id, PHASE_IDLE, || Some(true)),
                Some(LABEL_FIND)
            );
            assert_eq!(label_for_card(id, PHASE_IDLE, || Some(false)), None);
            assert_eq!(label_for_card(id, PHASE_IDLE, || None), None);
            // A running queue shows Stop Searching whatever the card.
            assert_eq!(
                label_for_card(id, PHASE_SEARCHING, || Some(false)),
                Some(LABEL_STOP)
            );
        }
        // Other texts never read the menu.
        assert_eq!(
            label_for_card(130_406, PHASE_IDLE, || panic!("read the menu")),
            None
        );
        assert_eq!(
            label_for_card(130_409, PHASE_IDLE, || panic!("read the menu")),
            Some(LABEL_SOLO)
        );
    }

    #[test]
    fn bots_on_starts_a_solo_match_against_bots_and_says_solo_only_otherwise() {
        let d = |p, card| decide_for_card(card, p, PHASE_IDLE, true, GameMode::BattleRoyale, true);
        assert_eq!(d(PLAYERS_ONE, Some(true)), Decision::PassBots);
        assert_eq!(
            d(PLAYERS_TWO, Some(true)),
            Decision::Dialog {
                text: T_BOTS_SOLO,
                request: REQ_NONE
            }
        );
        // Another card is the game's own; a running queue is not interrupted.
        assert_eq!(d(PLAYERS_ONE, Some(false)), Decision::Pass);
        assert_eq!(
            decide_for_card(
                Some(true),
                PLAYERS_ONE,
                PHASE_SEARCHING,
                true,
                GameMode::BattleRoyale,
                true
            ),
            Decision::Dialog {
                text: T_STOPPED,
                request: REQ_LEAVE
            }
        );
        // The labels of the row.
        assert_eq!(label_swap(130_002, PHASE_IDLE), Some(LABEL_BOTS));
        assert_eq!(label_swap(130_200, PHASE_IDLE), Some(LABEL_BOTS_ON));
        assert_eq!(label_swap(130_201, PHASE_IDLE), Some(LABEL_BOTS_OFF));
    }

    #[test]
    fn only_the_battle_royale_card_starts_our_queue() {
        use GameMode::BattleRoyale as BR;
        let br = Some(true);
        let other = Some(false);
        // Idle: our card queues, any other card (or an unreadable menu) is the game's own Commence.
        assert_eq!(
            decide_for_card(br, PLAYERS_ONE, PHASE_IDLE, true, BR, false),
            decide(PLAYERS_ONE, PHASE_IDLE, true, BR)
        );
        for card in [other, None] {
            assert_eq!(
                decide_for_card(card, PLAYERS_ONE, PHASE_IDLE, true, BR, false),
                Decision::Pass
            );
            assert_eq!(
                decide_for_card(card, PLAYERS_TWO, PHASE_IDLE, true, BR, false),
                Decision::Pass
            );
            assert_eq!(
                decide_for_card(
                    card,
                    PLAYERS_ONE,
                    PHASE_IDLE,
                    true,
                    GameMode::Offline,
                    false
                ),
                Decision::Pass
            );
        }
        // Queue active: the card does not matter.
        for card in [br, other, None] {
            for phase in [PHASE_SEARCHING, PHASE_MATCHED] {
                assert_eq!(
                    decide_for_card(card, PLAYERS_ONE, phase, true, BR, false),
                    decide(PLAYERS_ONE, phase, true, BR)
                );
            }
        }
    }

    #[test]
    fn the_battle_royale_card_texts_are_in_the_package() {
        let rows = include_str!("../../../../gamedata/nightbossmenu-rows.json");
        let texts = include_str!("../../../../gamedata/fmg-entries.json");
        assert!(rows.contains("\"expeditionNameId\": 131152"));
        assert!(rows.contains("\"descriptionId\": 131153"));
        assert!(texts.contains("\"131152\": \"Battle Royale\""));
        assert!(texts.contains("\"131153\": "));
    }

    #[test]
    fn the_arena_card_starts_the_arena_search_solo_duo_or_trio() {
        use crate::mm::PHASE_IDLE;
        let bro = GameMode::BattleRoyale;
        assert_eq!(
            decide_arena(Some(true), PLAYERS_ONE, PHASE_IDLE, true, bro, false),
            Some(Decision::Dialog {
                text: T_ARENA_SEARCHING,
                request: REQ_QUEUE_ARENA_SOLO
            })
        );
        assert_eq!(
            decide_arena(Some(true), PLAYERS_TWO, PHASE_IDLE, true, bro, false),
            Some(Decision::Dialog {
                text: T_ARENA_DUO,
                request: REQ_QUEUE_ARENA_DUO
            })
        );
        assert_eq!(
            decide_arena(Some(true), PLAYERS_THREE, PHASE_IDLE, true, bro, false),
            Some(Decision::Dialog {
                text: T_ARENA_TRIO,
                request: REQ_QUEUE_ARENA_TRIO
            })
        );
        assert_eq!(
            decide_arena(Some(true), PLAYERS_ONE, PHASE_IDLE, false, bro, false),
            Some(Decision::Dialog {
                text: T_OFF,
                request: REQ_NONE
            })
        );
        assert_eq!(
            decide_arena(Some(false), PLAYERS_ONE, PHASE_IDLE, true, bro, false),
            None,
            "another card"
        );
        assert_eq!(
            decide_arena(None, PLAYERS_ONE, PHASE_IDLE, true, bro, false),
            None,
            "menu unreadable"
        );
        assert_eq!(
            decide_arena(Some(true), PLAYERS_ONE, PHASE_SEARCHING, true, bro, false),
            None,
            "Stop Searching goes the Battle Royale way"
        );
        assert_eq!(
            decide_arena(
                Some(true),
                PLAYERS_ONE,
                PHASE_IDLE,
                true,
                GameMode::Offline,
                false
            ),
            None,
            "Offline: the game's own expedition"
        );
    }

    #[test]
    fn bots_on_makes_the_arena_card_a_match_against_bots_of_the_chosen_size() {
        use crate::mm::PHASE_IDLE;
        let bro = GameMode::BattleRoyale;
        for mm_on in [true, false] {
            assert_eq!(
                decide_arena(Some(true), PLAYERS_ONE, PHASE_IDLE, mm_on, bro, true),
                Some(Decision::PassArenaBots(1)),
                "no queue needed (mm {mm_on})"
            );
        }
        assert_eq!(
            decide_arena(Some(true), PLAYERS_TWO, PHASE_IDLE, true, bro, true),
            Some(Decision::PassArenaBots(2)),
            "ARENA-6: Duo = LOCAL + 1 bot ally vs 2 bots"
        );
        assert_eq!(
            decide_arena(Some(true), PLAYERS_THREE, PHASE_IDLE, true, bro, true),
            Some(Decision::PassArenaBots(3)),
            "ARENA-6: Trio = LOCAL + 2 bot allies vs 3 bots"
        );
        assert_eq!(
            decide_arena(Some(false), PLAYERS_ONE, PHASE_IDLE, true, bro, true),
            None,
            "the Battle Royale card keeps its own bots match"
        );
        assert_eq!(
            decide_arena(Some(true), PLAYERS_ONE, PHASE_SEARCHING, true, bro, true),
            None,
            "a running search is not a bots match"
        );
    }
}
