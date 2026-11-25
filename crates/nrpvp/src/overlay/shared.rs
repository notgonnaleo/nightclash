//! Plain values exchanged with the main loop (`game::watch`): the [`Snapshot`] it builds of the players and rules, and the
//! [`Command`]s it executes. **Nothing here touches game memory**; every command runs on the main loop through `run_command`.
//! R5 removed the ImGui panel and its queue; the developer console (`console`) is the only sender now.
//!
//! T-015: the snapshot lists every PlayerIns of the ChrSet ([`PlayerRow`]); commands can target one of them by address.

/// What the console (and the old overlay) can ask the main loop to do. Addresses are PlayerIns addresses taken from the
/// snapshot; the main loop re-validates them against a fresh ChrSet resolve before touching anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// Select this team candidate and write it to the local player.
    Set(u8),
    /// Restore the original value captured before the first write (local player).
    Restore,
    /// T-015: write `value` to this player's local copy of team_type.
    SetFor { addr: usize, value: u8 },
    /// T-015: restore this player's captured original value.
    RestoreFor { addr: usize },
    /// T-015: persist on/off for this player.
    PersistFor { addr: usize, on: bool },
    /// T-020: resume (`true`) or pause (`false`) the PvPvE mode. Ignored when the mode is off in the config.
    SetPvpve(bool),
    /// T-015: log `MARK #n: <text>`; `None` = from the hotkey.
    Mark(Option<String>),
    /// T-032 (dev only): select the next snap-back position candidate.
    PosCycle,
    /// T-032 (dev only): save the local player's values of every position candidate.
    PosSave,
    /// T-032 (dev only): write the saved values into the selected candidate, local player only.
    PosWrite,
    /// MM-2 (dev only, `mm_queue`): queue for a match in this mode. The main loop only forwards it to the Steam worker thread.
    MmQueue(crate::mm::Mode),
    /// ARENA-5 (dev only): queue for an Arena match in this mode (`mm_queue_arena_*`).
    MmQueueArena(crate::mm::Mode),
    /// MM-2 (dev only): leave the queue / the ready match.
    MmLeave,
    /// MM-3 (dev only, `mm_join`): one join-method attempt (buttons (a) (b) (c) (d)).
    MmJoin(crate::mm::join::JoinCmd),
    /// MM-3d (dev only, `mm_sc_create`): the "create SC lobby (test)" button.
    MmScCreate,
    /// MM-3f (dev only, `mm_quiet_invite`): the "test quiet invite" button.
    MmQuietTest,
    /// T-040 (dev only): the next n re-down tries fail ("forced by console"). Handled by the main loop next to the combat
    /// controller, not by `run_command`.
    ReviveFail(u32),
    /// T-040 follow-up (dev only): switch the get-up re-down experiment (`Core::early_redown`); handled like `ReviveFail`.
    EarlyRedown(bool),
    /// T-041 step 1 (dev only): switch the revive-bar write trace (`game::bartrace`); handled in the main loop like `ReviveFail`.
    BarTrace(bool),
    /// T-041 step 3 prototype (dev only): switch the revive block.
    ReviveBlock(bool),
    /// HAWK-3 (dev only): the hawk env swap (`game::hawk`); handled in the main loop like `ReviveFail`.
    Hawk(HawkCmd),
    /// OFF-1 (dev console): switch the game mode (`crate::features::mode`); handled in the main loop by the feature list.
    Mode(crate::features::mode::GameMode),
    /// OFF-4 (dev console): start or clear the game's NPC bots (`crate::features::bots`); handled in the main loop by the feature list.
    Bots(crate::features::bots::BotsCmd),
    /// M5 (dev console): show or set the play-area clock (`crate::features::clock`); handled in the main loop by the feature list.
    Clock(crate::features::clock::ClockCmd),
    /// M5-3 (dev console): end the current night now (`crate::features::nights`); handled in the main loop by the feature list.
    NightEnd,
    /// M5-4 (dev console): read an event flag, or write one of ours / the research list (`game::flags::console_cmd`).
    Flag(u32, Option<bool>),
    /// NET-5 research (dev only): call `exe+rva(arg)` on the main loop (`game::rawcall`).
    #[cfg(feature = "dev")]
    RawCall(u32, [u64; 4]),
    /// RW-3 E1 (dev only): drop one item at LOCAL's position with the game's drop function (`game::itemdrop`).
    #[cfg(feature = "dev")]
    ItemDrop(u32, u16, [i32; 3]),
    /// ARENA-3 research (dev only): `give <id> [qty] [slot]`: the game's add-item call on LOCAL's inventory (`game::invspy::give`).
    #[cfg(feature = "dev")]
    Give(u32, u32, Option<u8>),
    /// ARENA-3 (dev only): `arena_loadout apply` (`game::arenaloadout::apply`).
    #[cfg(feature = "dev")]
    ArenaLoadout,
    #[cfg(feature = "dev")]
    SavedLoadouts(crate::game::saved_loadouts::ConsoleCmd),
    /// RW-3 (dev only): `drop_gear [dry]`: what a defeat would drop, or drop it now without a defeat (`game::defeatdrop`).
    #[cfg(feature = "dev")]
    DropGear(bool),
    /// ARENA-0 (dev only): `arena_card [0|1|hook|unhook]` (`game::arenacard::Req` code).
    #[cfg(feature = "dev")]
    ArenaCard(u8),
}

/// HAWK-3 console commands (`hawk_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HawkCmd {
    /// Log where the env switch is and whose code it is (no swap).
    Env,
    /// Trace the hawk ids per character (`true`), with every id of the rider too (second `true`), or stop.
    Trace(bool, bool),
    /// Answer "route ended" to the rider for a moment.
    Drop,
    /// Start-hawk rides answer "tree hawk".
    Tree(bool),
    /// HAWK-4: tree mode with the stick hidden until a dive starts.
    Cruise(bool),
    /// HAWK-4: dive hold (units of 394) and dive length (ms).
    Dive(u32, u32),
    /// HAWK-4 polish: ride start mode (0 show the tree start animation, 1 skip it, 2 vanilla start then tree ride).
    Start(u8),
    /// HAWK-4 polish: the hawk's x1.3 speed during a dive.
    Boost(bool),
    /// FS7-5 probe: start a ride for LOCAL now (377 answered `kind`: 1 tree, 4 start) with no fall for `secs` seconds, after lifting
    /// LOCAL by the third value (metres; 0 = where it stands).
    Ride(u8, u32, u32),
    /// Everything off and the game's switch back.
    Off,
    /// Only the state line.
    Status,
}

/// Which PlayerIns is local (the first valid one in ChrSet entry order, T-020).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Local,
    Remote,
}

/// T-026: short form of a player id for the overlay: the first four hex digits, `0x6001..`; `?` if unreadable.
/// (ASCII only: the default ImGui font has no ellipsis glyph.)
pub(crate) fn short_id(id: Option<u64>) -> String {
    match id {
        None => "?".to_string(),
        Some(v) => {
            let hex = format!("{v:X}");
            if hex.len() > 4 {
                format!("0x{}..", &hex[..4])
            } else {
                format!("0x{hex}")
            }
        }
    }
}

/// T-026: how a player relates to the LOCAL player according to the rosters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Relation {
    Local,
    Ally,
    Rival,
}

/// One PlayerIns of the ChrSet, as plain values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlayerRow {
    /// ChrSet entry index.
    pub index: usize,
    pub addr: usize,
    pub role: Role,
    /// Why `role` is what it is ("first PlayerIns in the ChrSet", "not the local player").
    pub basis: String,
    /// Live ChrIns+0x74.
    pub team: Option<u8>,
    /// ChrSet entry bytes +8 and +9 (meaning UNVERIFIED).
    pub state8: u8,
    pub state9: u8,
    /// Original value captured before the first write to this player.
    pub original: Option<u8>,
    /// Persist value, if persist is on for this player.
    pub persist: Option<u8>,
    pub reverts_last_sec: u32,
    /// T-026: per-player id (ChrIns+0x78), if readable.
    pub id: Option<u64>,
    /// T-026: 0-based team number from the rosters (None when the rosters are unavailable).
    pub team_no: Option<u8>,
    /// T-026: relation to the local player (None when there is no local player).
    pub relation: Option<Relation>,
    pub reverts_total: u64,
}

/// T-020: state of the PvPvE mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PvpveState {
    /// Not enabled in the config (`pvpve = false`).
    #[default]
    Off,
    /// Enabled and applying.
    On,
    /// Enabled but paused (console `pvp 0`).
    Paused,
}

impl PvpveState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            PvpveState::Off => "OFF",
            PvpveState::On => "ON",
            PvpveState::Paused => "PAUSED",
        }
    }
}

/// The main loop's view of the game for the console, as plain values (no pointers are ever dereferenced from here).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
    /// T-040: forced re-down failures still to come (`revive_fail`).
    pub revive_fail_left: u32,
    /// T-040 follow-up: the get-up re-down experiment is on (`early_redown`).
    pub early_redown: bool,
    /// T-041 step 1: the revive-bar write trace is on (`bar_trace`) and how many writes it has captured.
    pub bar_trace: bool,
    pub bar_trace_hits: u64,
    /// T-041 step 3 prototype: the revive block is on (`revive_block`) and how many revives it stopped.
    pub revive_block: bool,
    pub revive_blocks: u32,
    /// WorldChrMan instance address, 0 = not live.
    pub wcm_instance: usize,
    /// PlayerIns-derived vtables known (RTTI recovered); writes are refused without them.
    pub vtables_known: bool,
    /// Resolved local PlayerIns address (ChrSet entry[0] rule).
    pub local_player: Option<usize>,
    /// Live `team_type` of the local player (ChrIns+0x74), read by the main loop.
    pub team_type: Option<u8>,
    /// Original value captured before the first write to this local player.
    pub original: Option<u8>,
    /// Currently selected candidate (what the write hotkey applies).
    pub selected: u8,
    /// Configured `team_candidates`.
    pub candidates: Vec<u8>,
    /// Persist is on for the local player.
    pub persist: bool,
    /// Reverts the game made in the last completed second of persist mode (all players).
    pub reverts_last_sec: u32,
    pub reverts_total: u64,
    /// Why the local player is not resolved, or empty.
    pub note: String,
    /// T-015: every valid PlayerIns of the ChrSet, entry order.
    pub players: Vec<PlayerRow>,
    /// T-015: how the local player was decided (empty when there is none).
    pub local_basis: String,
    /// T-015: MARK lines logged so far.
    pub marks: u32,
    /// T-020: PvPvE mode state, its team value and how many automatic writes were made this session.
    pub pvpve: PvpveState,
    pub pvpve_team: u8,
    pub pvpve_writes: u64,
    /// T-026: scheme label ("rivals" / "all"), team size, and a warning when the rosters fell back to free-for-all.
    pub pvpve_scheme: String,
    pub team_size: u8,
    pub roster_warning: String,
}
