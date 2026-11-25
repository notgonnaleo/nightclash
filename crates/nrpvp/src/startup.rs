//! Mod thread entry point: logging setup and the startup banner.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::{config, game, logging, sysinfo};

pub(crate) fn run() {
    // A panic here must never take the game down (and must never go unrecorded).
    let result = catch_unwind(AssertUnwindSafe(|| {
        let dll_dir = match sysinfo::dll_directory(crate::dll_handle()) {
            Some(d) => d,
            // Without our own directory there is nowhere sensible to put the log; bail quietly.
            None => return,
        };
        let setup = match logging::init(&dll_dir, config::peek_log_dir(&dll_dir).as_deref()) {
            Ok(s) => s,
            Err(_) => return,
        };
        if let Some(m) = banner(&setup) {
            // T-008 config (created with defaults if missing).
            let cfg = config::load(&dll_dir);
            // UW-1 (on by default): first, so the game's first display apply (window creation) already draws the full window.
            if cfg.ultrawide {
                game::ultrawide::install(m.base);
            }
            // MM-1: read/listen-only Steam probe, MM-2: the queue worker. One Steam thread serves both (one listener registration).
            // MM-4: the player build turns the queue on through `matchmaking` (config.rs forces every dev-only switch off there).
            let queue = cfg.mm_queue.then(|| {
                crate::mm::enable();
                crate::mm::worker::Setup {
                    need: u32::try_from(cfg.mm_max_players).unwrap_or(6),
                    room: cfg.private_code.clone(),
                    min: u32::try_from(cfg.mm_min_players).unwrap_or(2),
                    fill_wait: std::time::Duration::from_secs(cfg.mm_fill_wait_s),
                    // Player words in the player build; the detailed lines in developer mode.
                    friendly: !cfg.dev_mode,
                    // The game loop reads the local player once per watcher period; a few periods without a read = world unloaded.
                    world_stale: std::time::Duration::from_millis(
                        cfg.watch_interval_ms.saturating_mul(3).max(4000),
                    ),
                    join: cfg.mm_join,
                    join_auto: cfg.join_auto(),
                    // MM-3e: the host's mod creates the SC lobby at MATCH READY.
                    host_create: cfg.mm_host_create,
                    // MM-3f: skip the Steam invite window at the host's MATCH READY.
                    quiet_invite: cfg.mm_quiet_invite,
                    // MM-3g + MM-7: no click after Commence.
                    auto_start: cfg.mm_auto_start,
                    // MM-3d: the "create SC lobby (test)" dev button.
                    sc_create: cfg.mm_sc_create.then(|| cfg.sc_create_type()),
                    // The game build string the banner logs (exe FileVersion); TODO MM-4: a sturdier build id if needed.
                    game: m
                        .file_version
                        .clone()
                        .unwrap_or_else(|| crate::mm::worker::GAME_UNKNOWN.to_string()),
                }
            });
            // NET-2: the data channel takes its roster from the Seamless Co-op lobby, which the Steam thread watches: it runs for `net` too.
            if cfg.mm_probe || cfg.mm_queue || cfg.mm_join || cfg.net {
                // MM-3: `mm_join` or `mm_probe` also turn on the evidence logging (per-lobby membership lines) even without the queue worker.
                // MM-4: the player build keeps the callbacks the queue needs but not the probe's lobby dumps (both flags are dev only).
                crate::steam::probe::spawn(
                    queue,
                    cfg.dev_mode && (cfg.mm_join || cfg.mm_probe),
                    cfg.dev_mode,
                );
            }
            // NET-1/2: the mod-to-mod data channel (no Steam call until the game has initialised Steam). Developer builds: `net = true`;
            // player build: with matchmaking (config.rs).
            if cfg.net {
                crate::net::spawn();
            }
            // MM-3f: the invite-window suppression (hardware breakpoints, no patching). The config already turned it off when
            // mm_lobby_trace is on (both own the debug registers).
            if cfg.mm_quiet_invite && cfg.mm_join && cfg.mm_queue && !cfg.mm_lobby_trace {
                crate::steam::quietinvite::spawn(m.base, m.size);
            }
            // MM-3c: dev-only call-stack tracer for the creation of Seamless Co-op's lobby A (hardware breakpoints, no patching).
            #[cfg(feature = "dev")]
            if cfg.dev_mode && cfg.mm_lobby_trace {
                crate::steam::lobtrace::spawn();
            }
            // UI-0: dev-only text-id tracer (hardware breakpoints on the game's text lookup, no patching).
            // UI-2: matchmaking status as on-screen game messages (flags our Roundtable events wait on).
            if cfg.ui_messages && cfg.mm_queue {
                crate::mm::ui_msg::spawn(m.base);
            }
            // UI-2 spike: show our test messages through the flags our Roundtable events wait on.
            if cfg.dev_mode && cfg.ui_msg_test {
                game::flags::spawn_msg_test(m.base);
            }
            // UI-1 option 1: Commence Expedition drives the Battle Royale queue (shared hardware breakpoint, no patching).
            // The player build turns it on with matchmaking (config.rs).
            if cfg.br_menu && cfg.mm_queue {
                game::menuflow::spawn(m.base);
            }
            // UI-1 A/B/C: dev-only expedition-menu tracer (hardware breakpoints, no patching).
            #[cfg(feature = "dev")]
            if cfg.dev_mode && cfg.ui_menu_trace {
                game::menutrace::spawn(m.base, m.size);
            }
            #[cfg(feature = "dev")]
            if cfg.dev_mode && cfg.ui_text_trace {
                game::texttrace::spawn(m.base, m.size);
            }
            // UI-5 research: who asks for a toast's text (hardware breakpoint, no patching).
            #[cfg(feature = "dev")]
            if cfg.dev_mode && cfg.toast_trace {
                game::toasttrace::spawn(m.base, m.size);
            }
            // UI-1 step 0: dev-only fast flag-change logger (read-only).
            #[cfg(feature = "dev")]
            if cfg.dev_mode && cfg.ui_flag_watch {
                game::flagwatch::spawn(m.base);
            }
            // Several instances on one PC: close the engine's single-instance mutex (dev builds, dev mode, opt-in).
            #[cfg(feature = "dev")]
            if cfg.dev_mode && cfg.allow_multi_instance {
                game::multi_instance::spawn();
            }
            // CON-2: the developer console (named pipe for nrcon.exe). Dev builds, dev mode only.
            #[cfg(feature = "dev")]
            if cfg.dev_mode {
                crate::console::spawn(dll_dir.display().to_string());
            }
            // R3 / OFF-1: the feature list (the game mode is published here, before any menu can be pressed).
            let features = crate::modes::assemble(&cfg);
            // T-006 singleton discovery, T-007a RTTI + character discovery and the T-008 controls. Never
            // returns.
            game::run(&m, &cfg, features);
        }
    }));
    if result.is_err() {
        tracing::error!("nrpvp startup thread panicked");
    }
}

fn banner(setup: &logging::LogSetup) -> Option<sysinfo::GameModule> {
    tracing::info!(
        "nrpvp {} starting (T-006 singletons, T-007a discovery, T-008 team controls, T-013 overlay)",
        env!("CARGO_PKG_VERSION")
    );
    tracing::info!("unix time {}", sysinfo::unix_time_secs());
    tracing::info!(
        "log dir: {} (nrpvp.log + {}/)",
        setup.log_dir.display(),
        logging::SESSION_DIR_NAME
    );
    if let Some(note) = &setup.fallback_note {
        tracing::warn!("T-018: {note}; logging next to the DLL instead");
    }
    match &setup.session_path {
        Some(p) => tracing::info!("session log: {}", p.display()),
        None => tracing::info!(
            "session log: none (logging to {} only)",
            logging::LOG_FILE_NAME
        ),
    }

    match sysinfo::game_module() {
        Some(m) => {
            tracing::info!(
                "{}: base=0x{:X} size=0x{:X}{}",
                m.name,
                m.base,
                m.size,
                if m.is_fallback {
                    " (nightreign.exe not found by name, using the process main module)"
                } else {
                    ""
                }
            );
            tracing::info!("exe path: {}", m.path);
            match &m.file_version {
                Some(v) => tracing::info!("exe FileVersion: {v}"),
                None => {
                    tracing::warn!("exe FileVersion: unavailable (version resource not readable)")
                }
            }
            Some(m)
        }
        None => {
            tracing::warn!("could not resolve the game module (GetModuleHandle failed)");
            None
        }
    }
}
