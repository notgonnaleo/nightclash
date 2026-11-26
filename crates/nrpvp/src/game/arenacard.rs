//! ARENA-0 (dev only, console `arena_card`): make the Arena card (`NightBossMenuParam` row 17, `gamedata/nightbossmenu-rows.json`)
//! show in the Roundtable expedition list. Game 1.03.3 / exe 1.3.3.0, MEMORY_MAP §28.
//!
//! The list builder `exe+0x9E9330` shows rows 0..9 by position, then one card per byte `b` of the Everdark rotation list (row
//! `b + 10`), then row 100. The rotation list is a `vector<i8>` in `CSServerInterface` (FD4 slot `exe+0x3C17248`): the getter
//! `exe+0xAFED50` returns `instance + 0x30`, and `exe+0x9E5B10` copies `[+0x08, +0x10)` (begin, end) from it. So byte 7 in that
//! list shows row 17. The capacity at `+0x18` is `[HYPOTHESIS: MSVC vector layout, allocator first]`: the byte is only written
//! when `end < capacity`, never past it, and nothing is allocated here.

use std::sync::Mutex;

use super::mem;

/// `CSServerInterface` FD4 singleton slot (exe 1.3.3.0, `nrpvp.log` singleton list).
const RVA_SERVER_INTERFACE: usize = 0x3C1_7248;
/// The rotation list object inside it (the getter `exe+0xAFED50` returns `instance + 0x30`).
const ROTATION: usize = 0x30;
const BEGIN: usize = 0x08;
const END: usize = 0x10;
const CAP: usize = 0x18;
/// Row 17 = byte 7 (the list builder adds 10).
pub(crate) const ARENA_BYTE: u8 = 7;
/// A rotation list longer than this is not one (the game has 9 Everdark rows).
const MAX_LEN: usize = 64;

static LAST: Mutex<String> = Mutex::new(String::new());

fn record(line: String) {
    tracing::info!("{line}");
    *LAST
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = line;
}

/// The last answer (console reply).
pub(crate) fn reply() -> String {
    LAST.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// What to do with the list, from the bytes, the room left and the request (pure; tested).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    Nothing,
    /// Append the byte at `end` (room is there).
    Append,
    /// Remove the byte at this index (shift the rest down, end - 1).
    Remove(usize),
    /// Asked to add, but the list has no spare capacity.
    NoRoom,
}

pub(crate) fn plan(bytes: &[u8], room: usize, want: Option<bool>) -> Plan {
    let at = bytes.iter().position(|&b| b == ARENA_BYTE);
    match (want, at) {
        (Some(true), None) if room > 0 => Plan::Append,
        (Some(true), None) => Plan::NoRoom,
        (Some(false), Some(i)) => Plan::Remove(i),
        _ => Plan::Nothing,
    }
}

/// Our flag that warps this game's player into Heolstor's arena (event 99100900, `gamedata/emevd-arena.json`).
const FLAG_WARP: u32 = 10_007_500;
/// ARENA-2: our flag that turns this game's screen black at once with the player frozen (event 99100918): set at the warp and again
/// at the arrival (a map load may clear it), so nobody sees the move to the spawns; `arenarounds` fades back in.
const FLAG_BLACK: u32 = 10_007_518;
/// ARENA-1: how long LOCAL stands in the expedition of an Arena match before the warp.
const WARP_AFTER_MS: u64 = 3_000;
/// Heolstor's arrival point 18002200 in world metres (MEMORY_MAP §27, game 1.03.3) and how near LOCAL must be to count as arrived.
const ARRIVAL: [f32; 3] = [12.2, 1.3, 83.6];
const ARRIVED_WITHIN_M: f32 = 15.0;
/// How long LOCAL stands at the arrival point before the move to its spawn (a move right after a load is undone by the spawn, TRAPS).
const SETTLE_MS: u64 = 3_000;
/// Give up placing after this long past the warp (the warp failed or the map is another one).
const PLACE_TIMEOUT_MS: u64 = 60_000;
/// Spawns A (roster team 0) and B (team 1): 30 m apart on the arena's x axis (Leonardo 2026-11-08: 60 m was too far), facing each other
/// (MEMORY_MAP §27).
/// In this game yaw +90 faces -x (`teleport::quat_of_yaw`; seen live 2026-11-08: yaw 90 at A turned the player away from B), so A
/// (x -15) looks to +x with yaw -90 and B with yaw +90.
const SPAWNS: [([f32; 3], f32); 2] = [([-15.0, 3.0, 0.0], -90.0), ([15.0, 3.0, 0.0], 90.0)];

/// ARENA-1: where this game is in the Arena entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// No Arena match.
    Idle,
    /// An Arena match is on: warp once LOCAL has been in the expedition [`WARP_AFTER_MS`].
    WaitWarp,
    /// Warped: move LOCAL to its spawn once it has stood at the arrival point [`SETTLE_MS`].
    WaitArrival,
    /// Placed (or gave up) for this match.
    Done,
}

/// What the main loop does this tick (pure; tested).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Act {
    Nothing,
    Warp,
    Place,
    GiveUp,
}

/// One step of the Arena entry (pure). `held_ms`: how long the current condition has held (LOCAL in the expedition while
/// [`Entry::WaitWarp`], LOCAL at the arrival point while [`Entry::WaitArrival`]); `since_warp_ms`: time since the warp.
pub(crate) fn step(
    e: Entry,
    arena_match: bool,
    held_ms: Option<u64>,
    since_warp_ms: u64,
) -> (Entry, Act) {
    match (e, arena_match) {
        (_, false) => (Entry::Idle, Act::Nothing),
        (Entry::Idle, true) => (Entry::WaitWarp, Act::Nothing),
        (Entry::WaitWarp, true) if held_ms.is_some_and(|h| h >= WARP_AFTER_MS) => {
            (Entry::WaitArrival, Act::Warp)
        }
        (Entry::WaitArrival, true) if held_ms.is_some_and(|h| h >= SETTLE_MS) => {
            (Entry::Done, Act::Place)
        }
        (Entry::WaitArrival, true) if since_warp_ms >= PLACE_TIMEOUT_MS => {
            (Entry::Done, Act::GiveUp)
        }
        (e, true) => (e, Act::Nothing),
    }
}

/// ARENA-1: our flags that turn this game's camera (events 99100910..99100913, `gamedata/emevd-arena.json`: Set Camera Angle Y 0 /
/// 90 / 180 / 270), and the one each spawn uses so the camera looks at the other spawn. `[HYPOTHESIS: the event's Y angle uses the
/// same degrees as the player's yaw, so A (yaw -90) takes 270 and B (yaw 90) takes 90]`; check in game (`flag 1000751x 1`).
const CAMERA_FLAGS: [u32; 2] = [10_007_513, 10_007_511];

/// ARENA-5: teammates stand side by side, this far apart along z (slot 0 in the middle, slot 1 on +z, slot 2 on -z).
const MATE_GAP_M: f32 = 4.0;
/// ARENA-5 / ARENA-6: a player beaten while a teammate fights on is first put on its team's bench, this far out on x behind its own
/// spawn (105 m behind it), facing the centre like the spawn. ARENA-6: once that position has reached the other games the body is
/// disabled, which stops its position updates (mi-arena-hide-lab 2026-11-25): the others keep seeing it there, a figure too far off to
/// matter, while its own camera follows a teammate. The floor holds at ±120 m (the Arena bots' park, `features::bots::fill`).
const BENCH_X_M: f32 = 120.0;

fn side(team: Option<u8>) -> usize {
    usize::from(team.is_some_and(|t| t > 0))
}

fn slot_z(slot: u8) -> f32 {
    match slot {
        0 => 0.0,
        s if s % 2 == 1 => MATE_GAP_M * f32::from(s.div_ceil(2)),
        s => -MATE_GAP_M * f32::from(s / 2),
    }
}

/// The spawn of a roster team (pure): team 0 A, every other team B (`None` = unknown: A). ARENA-5: `slot` is the player's place in its
/// team (0-based, [`crate::mm::local_roster_slot`]); teammates stand [`MATE_GAP_M`] apart.
pub(crate) fn spawn_for(team: Option<u8>, slot: u8) -> ([f32; 3], f32) {
    let ([x, y, z], yaw) = SPAWNS[side(team)];
    ([x, y, z + slot_z(slot)], yaw)
}

/// ARENA-SPAWN: LOCAL's spawn now (its roster team and slot, as [`place`] uses them).
pub(crate) fn local_spawn() -> [f32; 3] {
    spawn_for(
        crate::mm::local_roster_team(),
        crate::mm::local_roster_slot().unwrap_or(0),
    )
    .0
}

/// ARENA-5: the bench of a roster team (pure): on the line of its spawn, [`BENCH_X_M`] from the centre, same facing.
pub(crate) fn bench_for(team: Option<u8>, slot: u8) -> ([f32; 3], f32) {
    let ([x, y, z], yaw) = spawn_for(team, slot);
    ([BENCH_X_M * x.signum(), y, z], yaw)
}

fn near(a: [f32; 3], b: [f32; 3], m: f32) -> bool {
    let d: f32 = (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum();
    d.sqrt() <= m
}

/// Main-loop state of [`tick`].
struct EntryState {
    entry: Entry,
    held_since: Option<std::time::Instant>,
    warped_at: Option<std::time::Instant>,
}
static STATE: Mutex<EntryState> = Mutex::new(EntryState {
    entry: Entry::Idle,
    held_since: None,
    warped_at: None,
});

/// Main loop, every tick. (1) The list hook holds hwbp slot 1 at the Roundtable while the queue is idle or searching (a menu reopened
/// mid-search keeps the Arena card); it gives the slot back once a match is found, when the quiet invite needs it. (2) ARENA-1: in an
/// Arena match, warp into Heolstor's arena 3 s into the expedition, then put LOCAL on its team's spawn, facing the other.
pub(crate) fn tick(base: usize, in_expedition: bool, local: bool, at_roundtable: bool) {
    let phase = crate::mm::phase();
    let menu_time = phase == crate::mm::PHASE_IDLE || phase == crate::mm::PHASE_SEARCHING;
    if !in_expedition && menu_time {
        if !hook::armed() {
            if let Err(e) = hook::arm(base) {
                hook::note_once(&e);
            }
        }
    } else if hook::armed() {
        hook::disarm();
    }
    // ARENA-3: remember the Sparring loadout while the player can still change it: at the Roundtable proper, never on a load screen
    // (the expedition start wipes the loadout before the expedition counts as entered; a capture there took the starting kit).
    super::arenaloadout::capture_tick(base, at_roundtable && menu_time && local);
    let arena = crate::mm::arena_match();
    let mut st = STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = std::time::Instant::now();
    let holds = match st.entry {
        Entry::WaitWarp => in_expedition && local,
        Entry::WaitArrival => {
            local
                && crate::game::teleport::local_world_pos()
                    .is_some_and(|p| near(p, ARRIVAL, ARRIVED_WITHIN_M))
        }
        _ => false,
    };
    if holds {
        if st.held_since.is_none() && st.entry == Entry::WaitArrival {
            if let Err(e) = crate::game::flags::set(base, FLAG_BLACK, true) {
                tracing::warn!("arena: flag {FLAG_BLACK} not set: {e}");
            }
            tracing::info!("arena: arrived; screen black until the spawn move is done");
        }
        st.held_since.get_or_insert(now);
    } else {
        st.held_since = None;
    }
    let held_ms = st.held_since.map(|t| t.elapsed().as_millis() as u64);
    let since_warp_ms = st.warped_at.map_or(0, |t| t.elapsed().as_millis() as u64);
    let (next, act) = step(st.entry, arena, held_ms, since_warp_ms);
    if next != st.entry {
        tracing::info!("arena: entry {:?} -> {next:?}", st.entry);
        st.held_since = None;
        st.entry = next;
    }
    match act {
        Act::Nothing => {}
        Act::Warp => {
            st.warped_at = Some(now);
            if let Err(e) = crate::game::flags::set(base, FLAG_BLACK, true) {
                tracing::warn!("arena: flag {FLAG_BLACK} not set: {e}");
            }
            match crate::game::flags::set(base, FLAG_WARP, true) {
                Ok(_) => {
                    tracing::info!("arena: flag {FLAG_WARP} ON: warping into Heolstor's arena")
                }
                Err(e) => tracing::warn!("arena: flag {FLAG_WARP} not set: {e}"),
            }
        }
        Act::Place => {
            place(base, false);
            // ARENA-3: level 15 and the captured loadout at max upgrade, on the game's thread.
            super::arenaloadout::apply(base);
        }
        Act::GiveUp => tracing::warn!(
            "arena: no arrival at Heolstor's arena within {} s of the warp; LOCAL left where it is",
            PLACE_TIMEOUT_MS / 1000
        ),
    }
}

/// ARENA-1 / ARENA-2: put LOCAL on its roster team's spawn, facing the other spawn, and turn the camera that way. Any distance (a
/// reset between rounds starts wherever the round ended). `force`: write the heading even over a tilted rotation (mid-attack).
/// `true` when the move was written.
pub(crate) fn place(base: usize, force: bool) -> bool {
    move_to(base, force, false)
}

/// ARENA-5: put LOCAL on its team's bench (beaten while a teammate fights on), facing the centre. `true` when the move was written.
pub(crate) fn bench(base: usize, force: bool) -> bool {
    move_to(base, force, true)
}

fn move_to(base: usize, force: bool, to_bench: bool) -> bool {
    let team = crate::mm::local_roster_team();
    let slot = crate::mm::local_roster_slot().unwrap_or(0);
    let (to, yaw) = if to_bench {
        bench_for(team, slot)
    } else {
        spawn_for(team, slot)
    };
    let what = if to_bench { "bench" } else { "spawn" };
    let args: Vec<String> = [to[0], to[1], to[2], yaw]
        .iter()
        .map(|v| v.to_string())
        .collect();
    let side = side(team);
    let moved = if force {
        crate::game::teleport::tp_force(&args)
    } else {
        crate::game::teleport::tp_far(&args)
    };
    match moved {
        Ok(r) => {
            if let Err(e) = crate::game::flags::set(base, CAMERA_FLAGS[side], true) {
                tracing::warn!("arena: camera flag {} not set: {e}", CAMERA_FLAGS[side]);
            }
            tracing::info!(
                "arena: roster team {team:?} slot {slot} -> {what} ({to:?}, yaw {yaw}): {r}"
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                "arena: roster team {team:?} slot {slot}: {what} move failed (force {force}): {e}"
            );
            false
        }
    }
}

/// ARENA-4: the warp into the arena has been asked for this match (LOCAL is in the arena or on its way there).
pub(crate) fn warped() -> bool {
    matches!(
        STATE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry,
        Entry::WaitArrival | Entry::Done
    )
}

/// ARENA-2: LOCAL stands on its spawn in the arena (the entry is done for this match).
pub(crate) fn placed() -> bool {
    STATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry
        == Entry::Done
}

/// Console request codes (`arena_card`, `arena_card 1`, `0`, `hook`, `unhook`).
pub(crate) const REQ_SHOW: u8 = 0;
pub(crate) const REQ_ADD: u8 = 1;
pub(crate) const REQ_REMOVE: u8 = 2;
pub(crate) const REQ_HOOK: u8 = 3;
pub(crate) const REQ_UNHOOK: u8 = 4;

/// Console `arena_card [0|1|hook|unhook]` (main loop).
pub(crate) fn console_cmd(base: usize, req: u8) {
    let r = match req {
        REQ_HOOK => hook::arm(base),
        REQ_UNHOOK => Ok(hook::disarm()),
        REQ_ADD => run(base, Some(true)),
        REQ_REMOVE => run(base, Some(false)),
        _ => run(base, None).map(|s| format!("{s}; {}", hook::state())),
    };
    record(match r {
        Ok(s) => format!("arena_card: {s}"),
        Err(e) => format!("arena_card: error: {e}"),
    });
}

/// The normal-card route: the list builder's first loop (`exe+0x9E9330`) takes rows by POSITION 0..9 (`inc esi; cmp esi, 0xA` then
/// `jl exe+0x9E93A0` at `exe+0x9E9665`). An execute breakpoint on that `jl`: when the loop is about to end (esi = 10), esi becomes
/// [`ROW17_POSITION`] and the loop runs once more, so row 17 is added like the first ten (not as an Everdark card); then esi = 18 and
/// the loop ends. Registers rbx / rdi are set right before the `jl` (the loop's own values), r12 / r13 / r14 / r15 are kept by the body.
mod hook {
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    use std::sync::Once;

    use windows::Win32::Foundation::STATUS_SINGLE_STEP;
    use windows::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, EXCEPTION_POINTERS,
    };

    use crate::dr::{EFLAGS_RF, EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};

    const RVA_LOOP_JL: usize = 0x9E_9665;
    const RVA_LOOP_TOP: usize = 0x9E_93A0;
    /// The loop's own end: after row position 9.
    const LOOP_END: u64 = 10;
    /// Position of row 17 in the sorted table (rows 0..16 come before it).
    const ROW17_POSITION: u64 = 17;
    /// The quiet-invite slot: free while no invite runs (checked before use).
    const SLOT: usize = crate::hwbp::SLOT_QUIET_VTABLE;

    static ADDR: AtomicUsize = AtomicUsize::new(0);
    static TOP: AtomicUsize = AtomicUsize::new(0);
    static HITS: AtomicU32 = AtomicU32::new(0);
    static ADDED: AtomicU32 = AtomicU32::new(0);
    static VEH: Once = Once::new();
    static ARMED: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn veh(info: *mut EXCEPTION_POINTERS) -> i32 {
        // SAFETY: Windows passes a valid EXCEPTION_POINTERS; both pointers are null-checked before use.
        unsafe {
            if info.is_null() {
                return EXCEPTION_CONTINUE_SEARCH;
            }
            let (rec, ctx) = ((*info).ExceptionRecord, (*info).ContextRecord);
            if rec.is_null() || ctx.is_null() || (*rec).ExceptionCode != STATUS_SINGLE_STEP {
                return EXCEPTION_CONTINUE_SEARCH;
            }
            let at = ADDR.load(Ordering::Relaxed);
            if at == 0 || (*ctx).Rip as usize != at {
                return EXCEPTION_CONTINUE_SEARCH;
            }
            HITS.fetch_add(1, Ordering::Relaxed);
            if (*ctx).Rsi & 0xFFFF_FFFF == LOOP_END {
                (*ctx).Rsi = ROW17_POSITION;
                (*ctx).Rip = TOP.load(Ordering::Relaxed) as u64;
                ADDED.fetch_add(1, Ordering::Relaxed);
            }
            (*ctx).EFlags |= EFLAGS_RF;
        }
        EXCEPTION_CONTINUE_EXECUTION
    }

    pub(super) fn arm(base: usize) -> Result<String, String> {
        if ARMED.load(Ordering::Relaxed) {
            return Ok(state());
        }
        if !crate::hwbp::is_free(SLOT) {
            return Err(format!("hwbp slot {SLOT} is in use"));
        }
        TOP.store(base + RVA_LOOP_TOP, Ordering::Relaxed);
        // SAFETY: `veh` has the PVECTORED_EXCEPTION_HANDLER signature and passes on everything that is not its own address.
        VEH.call_once(|| unsafe {
            let _ = AddVectoredExceptionHandler(1, Some(veh));
        });
        ADDR.store(base + RVA_LOOP_JL, Ordering::Relaxed);
        crate::hwbp::set(SLOT, base + RVA_LOOP_JL);
        ARMED.store(true, Ordering::Relaxed);
        Ok(format!("{}: open the expedition menu", state()))
    }

    pub(super) fn disarm() -> String {
        if ARMED.swap(false, Ordering::Relaxed) {
            crate::hwbp::clear(SLOT);
            // The address stays known to the handler (a late hit on another thread must still be ours).
        }
        state()
    }

    pub(super) fn armed() -> bool {
        ARMED.load(Ordering::Relaxed)
    }

    /// Log an arming failure once per distinct reason (the main loop retries every tick).
    pub(super) fn note_once(e: &str) {
        static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
        let mut last = LAST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *last != e {
            tracing::info!("arena_card: list hook not armed: {e}");
            *last = e.to_string();
        }
    }

    pub(super) fn state() -> String {
        let armed = if ARMED.load(Ordering::Relaxed) {
            "on"
        } else {
            "off"
        };
        format!(
            "hook {armed} (hwbp slot {SLOT} on exe+0x{RVA_LOOP_JL:X}), {} hit(s), row 17 added {} time(s)",
            HITS.load(Ordering::Relaxed),
            ADDED.load(Ordering::Relaxed)
        )
    }
}

fn run(base: usize, want: Option<bool>) -> Result<String, String> {
    let inst = mem::read::<usize>(base + RVA_SERVER_INTERFACE)
        .filter(|p| mem::is_ptr(*p))
        .ok_or("no CSServerInterface instance")?;
    let v = inst + ROTATION;
    let begin = mem::read::<usize>(v + BEGIN).ok_or("rotation begin unreadable")?;
    let end = mem::read::<usize>(v + END).ok_or("rotation end unreadable")?;
    let cap = mem::read::<usize>(v + CAP).ok_or("rotation capacity unreadable")?;
    let len = end
        .checked_sub(begin)
        .filter(|n| *n <= MAX_LEN)
        .ok_or_else(|| format!("not a list: begin {begin:#X} end {end:#X} cap {cap:#X}"))?;
    let bytes = if len == 0 {
        Vec::new()
    } else {
        let mut out = vec![0u8; len];
        if !mem::copy_out(begin, &mut out) {
            return Err(format!("rotation bytes at {begin:#X} unreadable"));
        }
        out
    };
    let room = if begin != 0 && cap >= end {
        cap - end
    } else {
        0
    };
    let shown = |b: &[u8]| -> String {
        let rows: Vec<String> = b
            .iter()
            .map(|x| format!("{x} (row {})", i32::from(*x as i8) + 10))
            .collect();
        format!("[{}]", rows.join(", "))
    };
    let before = format!(
        "list at {v:#X}: {} byte(s) {} begin {begin:#X} end {end:#X} cap {cap:#X} (room {room})",
        bytes.len(),
        shown(&bytes)
    );
    match plan(&bytes, room, want) {
        Plan::Nothing => Ok(before),
        Plan::NoRoom if begin == 0 && end == 0 && cap == 0 => {
            // Offline / no server rotation: the list was never allocated. Ask the list's own allocator (`v + 0`, the one the
            // game's copy `exe+0x9E5B10` calls as `[vtable+0x50](allocator, size, 1)`) for a buffer, so a later free by the
            // game frees its own memory.
            let p = allocate(v, ALLOC_BYTES)?;
            mem::write_bytes(p, &[ARENA_BYTE])?;
            mem::write_bytes(v + BEGIN, &p.to_le_bytes())?;
            mem::write_bytes(v + CAP, &(p + ALLOC_BYTES).to_le_bytes())?;
            write_end(v, p + 1)?;
            Ok(format!(
                "{before}; allocated {ALLOC_BYTES} bytes at {p:#X} with the list's allocator, added byte {ARENA_BYTE} (row 17, Arena): open the expedition menu"
            ))
        }
        Plan::NoRoom => Err(format!(
            "{before}; no spare capacity, byte {ARENA_BYTE} not added"
        )),
        Plan::Append => {
            mem::write_bytes(end, &[ARENA_BYTE])?;
            write_end(v, end + 1)?;
            Ok(format!(
                "{before}; added byte {ARENA_BYTE} (row 17, Arena): reopen the expedition menu"
            ))
        }
        Plan::Remove(i) => {
            let rest: Vec<u8> = bytes[i + 1..].to_vec();
            for (k, b) in rest.iter().enumerate() {
                mem::write_bytes(begin + i + k, &[*b])?;
            }
            write_end(v, end - 1)?;
            Ok(format!("{before}; removed byte {ARENA_BYTE}"))
        }
    }
}

/// Bytes asked from the game's allocator for an empty list (room for the 9 Everdark bytes and ours).
const ALLOC_BYTES: usize = 16;
/// The allocate slot of the list allocator's vtable (`exe+0x9E5B6A`: `call [rax+0x50]` with rdx = size, r8d = 1).
const VT_ALLOCATE: usize = 0x50;

/// `allocator->allocate(size, 1)` through the list's own allocator object at `v + 0`.
fn allocate(v: usize, size: usize) -> Result<usize, String> {
    let alloc = mem::read::<usize>(v)
        .filter(|p| mem::is_ptr(*p))
        .ok_or("the list has no allocator")?;
    let vt = mem::read::<usize>(alloc)
        .filter(|p| mem::is_ptr(*p))
        .ok_or("allocator vtable unreadable")?;
    let f = mem::read::<usize>(vt + VT_ALLOCATE)
        .filter(|p| mem::is_ptr(*p))
        .ok_or("allocator allocate slot unreadable")?;
    // SAFETY: `f` is the allocate entry of the game's own allocator vtable, called with the arguments the game itself passes at
    // exe+0x9E5B6A (this, size, 1); it returns a heap pointer or null. Dev-only research path.
    let call: unsafe extern "system" fn(usize, usize, u32) -> usize =
        unsafe { std::mem::transmute(f) };
    let p = unsafe { call(alloc, size, 1) };
    if mem::is_ptr(p) {
        Ok(p)
    } else {
        Err(format!("allocator returned {p:#X}"))
    }
}

fn write_end(v: usize, end: usize) -> Result<(), String> {
    mem::write_bytes(v + END, &end.to_le_bytes()).map_err(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arena_byte_is_added_only_with_room_and_only_once() {
        assert_eq!(plan(&[0, 3], 2, Some(true)), Plan::Append);
        assert_eq!(plan(&[], 1, Some(true)), Plan::Append);
        assert_eq!(plan(&[0, 3], 0, Some(true)), Plan::NoRoom);
        assert_eq!(plan(&[0, 7], 2, Some(true)), Plan::Nothing);
        assert_eq!(plan(&[0, 7, 3], 0, Some(false)), Plan::Remove(1));
        assert_eq!(plan(&[0, 3], 0, Some(false)), Plan::Nothing);
        assert_eq!(plan(&[0, 3], 5, None), Plan::Nothing);
    }

    #[test]
    fn the_arena_entry_warps_once_then_places_once() {
        use Act::*;
        use Entry::*;
        assert_eq!(step(Idle, false, None, 0), (Idle, Nothing));
        assert_eq!(step(Idle, true, None, 0), (WaitWarp, Nothing));
        assert_eq!(step(WaitWarp, true, Some(2_999), 0), (WaitWarp, Nothing));
        assert_eq!(
            step(WaitWarp, true, None, 0),
            (WaitWarp, Nothing),
            "not in the expedition yet"
        );
        assert_eq!(step(WaitWarp, true, Some(3_000), 0), (WaitArrival, Warp));
        assert_eq!(
            step(WaitArrival, true, None, 10_000),
            (WaitArrival, Nothing),
            "still loading"
        );
        assert_eq!(step(WaitArrival, true, Some(3_000), 12_000), (Done, Place));
        assert_eq!(step(WaitArrival, true, None, 60_000), (Done, GiveUp));
        assert_eq!(
            step(Done, true, Some(99_999), 0),
            (Done, Nothing),
            "once per match"
        );
        assert_eq!(
            step(Done, false, None, 0),
            (Idle, Nothing),
            "the match is over"
        );
        assert_eq!(
            step(WaitWarp, false, Some(5_000), 0),
            (Idle, Nothing),
            "a BR match never warps"
        );
    }

    #[test]
    fn team_zero_takes_spawn_a_the_other_team_spawn_b_facing_each_other() {
        assert_eq!(spawn_for(Some(0), 0), ([-15.0, 3.0, 0.0], -90.0));
        assert_eq!(spawn_for(Some(1), 0), ([15.0, 3.0, 0.0], 90.0));
        assert_eq!(spawn_for(None, 0), spawn_for(Some(0), 0));
        // Both spawns are within one `tp` (100 m) of the arrival point.
        for (p, _) in SPAWNS {
            assert!(near(p, ARRIVAL, 100.0));
        }
    }

    #[test]
    fn teammates_stand_side_by_side_and_every_spawn_is_its_own() {
        assert_eq!(spawn_for(Some(0), 1), ([-15.0, 3.0, 4.0], -90.0));
        assert_eq!(spawn_for(Some(0), 2), ([-15.0, 3.0, -4.0], -90.0));
        assert_eq!(spawn_for(Some(1), 2), ([15.0, 3.0, -4.0], 90.0));
        let mut all = Vec::new();
        for team in [Some(0), Some(1)] {
            for slot in 0..3 {
                let (p, _) = spawn_for(team, slot);
                assert!(near(p, ARRIVAL, 100.0), "{team:?} {slot}: within one tp");
                assert!(
                    !all.iter().any(|q| near(p, *q, 3.0)),
                    "{team:?} {slot}: apart"
                );
                all.push(p);
            }
        }
    }

    #[test]
    fn a_bench_is_behind_its_own_spawn_away_from_the_fight() {
        for team in [Some(0), Some(1)] {
            for slot in 0..3 {
                let (s, yaw) = spawn_for(team, slot);
                let (b, byaw) = bench_for(team, slot);
                assert_eq!(byaw, yaw, "same facing: the centre");
                assert_eq!((b[1], b[2]), (s[1], s[2]));
                assert!(b[0].abs() > s[0].abs() + 20.0 && b[0].signum() == s[0].signum());
            }
        }
    }
}
