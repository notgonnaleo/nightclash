//! One test session: stage the mod into a run folder, preflight, launch through me3, check me3's attach config, watch the game,
//! stop it. State lives in the run folder (`session.json`), so separate `nro` invocations (and the MCP server) share it.
//!
//! Run folder layout:
//! ```text
//! runs\<id>\mod\            staged nrpvp.dll / .pdb / nrpvp.toml / gamedata\ (the mod logs next to its DLL: mod\nrpvp.log, mod\logs\)
//! runs\<id>\profile.me3.toml
//! runs\<id>\me3.log         me3's stdout + stderr (attach config line = check L1)
//! runs\<id>\preflight.json  the report the launch was decided on
//! runs\<id>\console.log     every console line nro sent and its reply
//! runs\<id>\watch.log       watchdog: processes, the game's TCP endpoints, the end-of-run save check
//! runs\<id>\shots\*.png
//! runs\<id>\session.json
//! ```

use nro_core::me3log;
use nro_core::pins::Pins;
use nro_core::preflight::{self, Facts, Report};
use nro_core::profile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::{console, sys};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub id: String,
    pub dir: String,
    pub me3_pid: Option<u32>,
    pub game_pid: Option<u32>,
    pub watch_pid: Option<u32>,
    /// "staged" | "refused" | "launching" | "running" | "ended" | "aborted"
    pub phase: String,
    pub note: String,
    pub started_unix: u64,
    /// Vanilla save files and their hashes before launch.
    pub vanilla_saves: BTreeMap<String, String>,
    /// Set for an emulated-Steam instance (emu.rs).
    #[serde(default)]
    pub emu: Option<crate::emu::EmuInfo>,
}

pub fn home() -> PathBuf {
    std::env::var_os("NRO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            strip_unc(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .canonicalize()
                    .unwrap_or_else(|_| ".".into()),
            )
        })
}

fn strip_unc(p: PathBuf) -> PathBuf {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(p)
}

pub fn pins() -> Result<Pins, String> {
    // NRO_PINS_FILE: another pins file in pins\ (a newer me3 under test), by file name.
    let name = std::env::var("NRO_PINS_FILE").unwrap_or_else(|_| "nightreign.toml".into());
    let file = home().join("pins").join(name);
    match fs::read_to_string(&file) {
        Ok(t) => Pins::parse(&t),
        Err(_) => Pins::parse(nro_core::pins::EMBEDDED),
    }
}

pub fn runs_dir() -> PathBuf {
    home().join("runs")
}

impl State {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join("session.json")
    }
    pub fn load(dir: &Path) -> Result<Self, String> {
        let t =
            fs::read_to_string(Self::path(dir)).map_err(|e| format!("{}: {e}", dir.display()))?;
        serde_json::from_str(&t).map_err(|e| e.to_string())
    }
    pub fn save(&self) -> Result<(), String> {
        let dir = PathBuf::from(&self.dir);
        fs::write(
            Self::path(&dir),
            serde_json::to_string_pretty(self).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        // The default target of `nro cmd` etc. is the run that was staged last (several instances save their own state later).
        if self.phase == "staged" {
            fs::write(runs_dir().join("current.txt"), &self.dir).map_err(|e| e.to_string())?;
            if let Some(e) = &self.emu {
                let mut g = Self::group();
                g.insert(e.n.to_string(), self.dir.clone());
                fs::write(
                    runs_dir().join("instances.json"),
                    serde_json::to_string_pretty(&g).unwrap(),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    /// Instance number → run folder of the emulated instances (`runs\instances.json`).
    pub fn group() -> BTreeMap<String, String> {
        fs::read_to_string(runs_dir().join("instances.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }
    /// The state of instance `n` (`--i N`).
    pub fn instance(n: u32) -> Result<Self, String> {
        let d = Self::group()
            .get(&n.to_string())
            .cloned()
            .ok_or_else(|| format!("no instance {n} (nro launch --emulated {n})"))?;
        Self::load(Path::new(&d))
    }
    pub fn current() -> Result<Self, String> {
        if let Some(n) = std::env::var("NRO_I").ok().and_then(|v| v.parse().ok()) {
            return Self::instance(n);
        }
        let dir = fs::read_to_string(runs_dir().join("current.txt"))
            .map_err(|_| "no session yet (nro launch)".to_string())?;
        Self::load(Path::new(dir.trim()))
    }
    pub fn dir(&self) -> PathBuf {
        PathBuf::from(&self.dir)
    }
    /// The game process, if it is still running. The image must still be `nightreign.exe`: Windows reuses pids, and a stale
    /// session's pid can belong to another program by now (seen 2026-10-27: a powershell.exe), which `stop` would otherwise kill.
    pub fn live_pid(&self) -> Option<u32> {
        self.game_pid.filter(|&p| {
            sys::image_path(p).is_some_and(|i| {
                i.rsplit('\\')
                    .next()
                    .is_some_and(|n| n.eq_ignore_ascii_case("nightreign.exe"))
            })
        })
    }
    pub fn log(&self, file: &str, line: &str) {
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir().join(file))
        {
            let _ = writeln!(f, "{} {line}", now_stamp());
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn now_stamp() -> String {
    let s = now_unix();
    let (h, m, sec) = ((s / 3600) % 24, (s / 60) % 60, s % 60);
    format!("{h:02}:{m:02}:{sec:02}Z")
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let t = to.join(e.file_name());
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir(&e.path(), &t)?;
        } else {
            fs::copy(e.path(), &t).map_err(|er| format!("copy {}: {er}", e.path().display()))?;
        }
    }
    Ok(())
}

/// Options for one launch.
#[derive(Debug, Clone, Default)]
pub struct Stage {
    /// A short name for the run folder.
    pub name: String,
    /// The mod build to test (folder with nrpvp.dll, nrpvp.toml, gamedata\). Default: `<mod repo>\dist`.
    pub mod_dist: Option<PathBuf>,
    /// Keys written over the staged nrpvp.toml.
    pub config: BTreeMap<String, toml::Value>,
    /// Load the game-data package (default true when the dist has one).
    pub no_gamedata: bool,
    /// Emulated-Steam instance number (1..=4); `None` = the normal real-Steam launch.
    pub emulated: Option<u32>,
}

/// The keys every orchestrated run forces (preflight P7 checks the result). A real-Steam run: the public queue off. An emulated
/// run may turn the queue on (its lobbies stay on the LAN-only emulator, MULTI-INSTANCE §20), but the joiner never opens a
/// `steam://` link (the shell would start the real Steam client).
fn forced_config(emulated: bool) -> BTreeMap<String, toml::Value> {
    let mut c = BTreeMap::new();
    c.insert("dev_mode".into(), toml::Value::Boolean(true));
    if emulated {
        c.insert("mm_join_auto".into(), toml::Value::String("invite".into()));
    } else {
        c.insert("mm_queue".into(), toml::Value::Boolean(false));
        c.insert("br_menu".into(), toml::Value::Boolean(false));
    }
    c
}

/// The staged config turns the queue on (`mm_queue`), so the launch waits for the emulated id in the mod log.
pub fn queue_on(st: &State) -> bool {
    fs::read_to_string(st.dir().join("mod").join("nrpvp.toml"))
        .ok()
        .and_then(|t| toml::from_str::<toml::Table>(&t).ok())
        .and_then(|t| t.get("mm_queue").and_then(|v| v.as_bool()))
        == Some(true)
}

/// Runtime identity check of an emulated instance (MULTI-INSTANCE §20): the mod log's own Steam id is the instance's emulated id.
pub fn check_identity(st: &State) -> Result<u64, String> {
    let e = st.emu.as_ref().ok_or("not an emulated instance")?;
    let log = fs::read_to_string(st.dir().join("mod").join("nrpvp.log")).unwrap_or_default();
    preflight::queue_identity(&log, e.steamid)
}

/// Console lines that start or join a matchmaking queue.
fn is_queue_line(line: &str) -> bool {
    let w = line.split_whitespace().next().unwrap_or("");
    w.starts_with("mm_queue") || w == "mm_join"
}

pub fn stage(pins: &Pins, opt: &Stage) -> Result<State, String> {
    let id = format!(
        "{}-{}",
        now_unix(),
        if opt.name.is_empty() {
            "run"
        } else {
            &opt.name
        }
    );
    let dir = runs_dir().join(&id);
    let m = dir.join("mod");
    fs::create_dir_all(&m).map_err(|e| e.to_string())?;
    let dist = opt.mod_dist.clone().unwrap_or_else(|| home().join("dist"));
    for f in ["nrpvp.dll", "nrpvp.pdb"] {
        let src = dist.join(f);
        if src.is_file() {
            fs::copy(&src, m.join(f)).map_err(|e| format!("copy {}: {e}", src.display()))?;
        } else if f == "nrpvp.dll" {
            return Err(format!(
                "{} missing (build the mod first: scripts\\build.ps1)",
                src.display()
            ));
        }
    }
    let base = fs::read_to_string(dist.join("nrpvp.toml")).unwrap_or_default();
    let mut cfg = opt.config.clone();
    cfg.extend(forced_config(opt.emulated.is_some()));
    if opt.emulated.is_some() {
        // The mod closes the engine mutex and dearxan's shared section so the next instance can start (game/multi_instance.rs).
        cfg.insert("allow_multi_instance".into(), toml::Value::Boolean(true));
    }
    fs::write(
        m.join("nrpvp.toml"),
        nro_core::scenario::apply_config(&base, &cfg),
    )
    .map_err(|e| e.to_string())?;
    let gd = dist.join("gamedata");
    let with_gd = !opt.no_gamedata && gd.join("msg").is_dir();
    if with_gd {
        copy_dir(&gd, &m.join("gamedata"))?;
    }
    let emu = match opt.emulated {
        Some(n) => Some(crate::emu::stage_instance(pins, n)?),
        None => None,
    };
    let nrsc = emu
        .as_ref()
        .map(|e| PathBuf::from(&e.game_dir))
        .unwrap_or_else(|| PathBuf::from(&pins.game_dir))
        .join("SeamlessCoop")
        .join("nrsc.dll");
    let gdp = m.join("gamedata").display().to_string();
    let prof = if std::env::var_os("NRO_NO_SC").is_some() {
        // NET-5 experiment: no Seamless Co-op (NETWORKING.md). Emulated instances only; the staged folder keeps its SC copy unused.
        profile::text_no_sc(
            &m.join("nrpvp.dll").display().to_string(),
            with_gd.then_some(gdp.as_str()),
        )
    } else {
        profile::text(
            &nrsc.display().to_string(),
            &m.join("nrpvp.dll").display().to_string(),
            with_gd.then_some(gdp.as_str()),
        )
    };
    fs::write(dir.join("profile.me3.toml"), prof).map_err(|e| e.to_string())?;
    let st = State {
        id,
        dir: dir.display().to_string(),
        phase: "staged".into(),
        emu,
        started_unix: now_unix(),
        ..Default::default()
    };
    st.save()?;
    Ok(st)
}

pub fn vanilla_saves() -> Option<BTreeMap<String, String>> {
    let root = PathBuf::from(std::env::var_os("APPDATA")?).join("Nightreign");
    let mut out = BTreeMap::new();
    for d in fs::read_dir(&root).ok()?.flatten() {
        if !d.path().is_dir() {
            continue;
        }
        for f in fs::read_dir(d.path()).ok()?.flatten() {
            let n = f.file_name().to_string_lossy().to_ascii_lowercase();
            if n.starts_with("nr0000.sl2") {
                out.insert(f.path().display().to_string(), sys::sha256(&f.path())?);
            }
        }
    }
    Some(out)
}

/// The me3 command line nro uses (P1: the only one).
fn me3_args(pins: &Pins, st: &State) -> Vec<String> {
    let mut v = vec![
        "launch".into(),
        "-g".into(),
        "nightreign".into(),
        "-e".into(),
        game_exe(pins, st).display().to_string(),
        "-p".into(),
        st.dir().join("profile.me3.toml").display().to_string(),
    ];
    if st.emu.is_some() {
        // No Steam client to initialise (preflight P3 allows exactly this for an emulated instance).
        v.extend([
            "--skip-steam-init".into(),
            "true".into(),
            "--no-boot-boost".into(),
            "true".into(),
        ]);
    }
    v
}

/// The `nightreign.exe` this run starts (the instance's own hard-linked copy when emulated).
fn game_exe(pins: &Pins, st: &State) -> PathBuf {
    st.emu
        .as_ref()
        .map(|e| e.exe())
        .unwrap_or_else(|| PathBuf::from(&pins.game_dir).join("nightreign.exe"))
}

pub fn facts(pins: &Pins, st: &State) -> Facts {
    let dir = st.dir();
    let profile = fs::read_to_string(dir.join("profile.me3.toml")).unwrap_or_default();
    let all = me3_args(pins, st);
    let me3 = PathBuf::from(&pins.me3_dir).join("me3.exe");
    let me3_version = Command::new(&me3)
        .arg("--version")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    Facts {
        game_hashes: pins
            .sha256
            .keys()
            .map(|k| {
                (
                    k.clone(),
                    sys::sha256(&PathBuf::from(&pins.game_dir).join(k)),
                )
            })
            .collect(),
        me3_hashes: pins
            .me3
            .files
            .keys()
            .map(|k| {
                (
                    k.clone(),
                    sys::sha256(&PathBuf::from(&pins.me3_dir).join(k)),
                )
            })
            .collect(),
        me3_version,
        processes: sys::processes(),
        profile_files: profile::paths(&profile)
            .into_iter()
            .map(|p| (p.clone(), Path::new(&p).exists()))
            .collect(),
        profile,
        me3_args: all[7..].to_vec(),
        mod_toml: fs::read_to_string(dir.join("mod").join("nrpvp.toml")).ok(),
        vanilla_saves_hashed: vanilla_saves().is_some(),
        emulated: st.emu.as_ref().map(|e| nro_core::preflight::Emulated {
            main_ini: fs::read_to_string(
                PathBuf::from(&e.game_dir)
                    .join("steam_settings")
                    .join("configs.main.ini"),
            )
            .ok(),
        }),
    }
}

pub fn preflight(pins: &Pins, st: &State) -> Report {
    let r = preflight::evaluate(pins, &facts(pins, st));
    let _ = fs::write(
        st.dir().join("preflight.json"),
        serde_json::to_string_pretty(&r).unwrap(),
    );
    r
}

/// Preflight, then launch. Refuses (and changes nothing else) when any check fails.
/// Our own stdin/stdout/stderr must not be inherited by me3, the game or the watchdog: a shell or an MCP client reading our output
/// would otherwise wait for those long-lived processes to close it.
fn no_std_inherit() {
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    for h in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: clearing the inherit flag on our own standard handles; invalid handles make the call fail harmlessly.
        unsafe { SetHandleInformation(GetStdHandle(h), HANDLE_FLAG_INHERIT, 0) };
    }
}

/// The pid in me3's `attaching to process pid=N` line (colour codes and all).
/// The pid in me3's "attaching to process pid=N" line. me3 colours its log even into a file (`pid\x1b[0m\x1b[2m=\x1b[0m9632`): the
/// escape codes are stripped first, else the "0" of "\x1b[0m" was read as the pid (2026-10-27: game_pid 0, every launch killed).
fn attach_pid(log: &str) -> Option<u32> {
    let mut plain = String::with_capacity(log.len());
    let mut chars = log.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI: ESC [ params final-byte (0x40..=0x7E); OSC (ESC ]) ends at BEL.
            match chars.next() {
                Some('[') => {
                    for d in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&d) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    for d in chars.by_ref() {
                        if d == '\x07' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            plain.push(c);
        }
    }
    let rest = &plain[plain.find("attaching to process")?..];
    let rest = &rest[rest.find("pid=")? + 4..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok().filter(|&p| p != 0)
}

pub fn launch(pins: &Pins, st: &mut State) -> Result<Report, String> {
    no_std_inherit();
    let r = preflight(pins, st);
    if !r.ok {
        st.phase = "refused".into();
        st.note = r
            .checks
            .iter()
            .filter(|c| !c.ok)
            .map(|c| format!("{}: {}", c.id, c.detail))
            .collect::<Vec<_>>()
            .join(" | ");
        st.save()?;
        return Ok(r);
    }
    st.vanilla_saves = vanilla_saves().unwrap_or_default();
    let dir = st.dir();
    let log = fs::File::create(dir.join("me3.log")).map_err(|e| e.to_string())?;
    let log2 = log.try_clone().map_err(|e| e.to_string())?;
    // Emulated instance: point Steam's ActiveProcess key at the emulator for the start, put it back once the game has loaded it.
    if let Some(e) = st.emu.as_mut() {
        crate::emu::redirect(e, std::process::id())?;
        st.save()?;
    }
    let mut cmd = Command::new(PathBuf::from(&pins.me3_dir).join("me3.exe"));
    cmd.args(me3_args(pins, st))
        .current_dir(&pins.game_dir)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log2)
        .creation_flags(CREATE_NO_WINDOW);
    let child = cmd
        .spawn()
        .map_err(|e| format!("could not start me3: {e}"))?;
    st.me3_pid = Some(child.id());
    st.phase = "launching".into();
    st.save()?;

    // The game process: the nightreign.exe that was not there at preflight (P5 guaranteed none was).
    let exe = game_exe(pins, st)
        .display()
        .to_string()
        .to_ascii_lowercase();
    let t0 = Instant::now();
    let pid = loop {
        if let Some(p) = sys::processes().into_iter().find(|p| {
            p.name.eq_ignore_ascii_case("nightreign.exe")
                && p.path.as_deref().map(str::to_ascii_lowercase) == Some(exe.clone())
        }) {
            break p.pid;
        }
        // me3 names the pid it attaches to; a game that dies right after the attach is never seen by the process poll.
        if let Some(p) = fs::read_to_string(dir.join("me3.log"))
            .ok()
            .and_then(|t| attach_pid(&t))
        {
            break p;
        }
        if t0.elapsed() > Duration::from_secs(90) {
            st.phase = "aborted".into();
            st.note = "the game process did not appear within 90 s (see me3.log)".into();
            st.save()?;
            return Err(st.note.clone());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let exit_handle = sys::open_for_exit(pid);
    if st.emu.is_some() && std::env::var_os("NRO_DEBUG_ATTACH").is_some() {
        // Diagnostic (S2): what does the second instance crash on?
        let r = sys::first_crash(pid, 20_000);
        st.log("watch.log", &format!("debug attach: {r}"));
    }
    st.game_pid = Some(pid);
    st.save()?;
    if st.emu.is_some() {
        // P-module: wait for the instance's own steamclient64.dll in the game, then give the registry back. Else kill (fail closed).
        let t2 = Instant::now();
        let mut exit_seen = None;
        let ok = loop {
            let mods = sys::modules(pid);
            if st
                .emu
                .as_ref()
                .is_some_and(|e| crate::emu::module_ok(&mods, &e.game_dir))
            {
                break true;
            }
            // Gone = the exit code is there. Not `!alive`: right after me3's attach, opening the process by pid can fail for a moment
            // while it runs (seen 2026-10-27: "alive=false, 0 modules" with the game up), which killed a good launch.
            exit_seen = if exit_handle != 0 {
                sys::exit_code(exit_handle)
            } else {
                (!sys::alive(pid)).then_some(0)
            };
            if t2.elapsed() > Duration::from_secs(60) || exit_seen.is_some() {
                break false;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        if let Some(e) = st.emu.as_mut() {
            let _ = crate::emu::restore(e);
        }
        if !ok {
            let mods = sys::modules(pid);
            st.log(
                "watch.log",
                &format!(
                    "P-module failed: alive={}, {} modules, steamclient modules {:?}",
                    sys::alive(pid),
                    mods.len(),
                    mods.iter()
                        .filter(|m| m.to_ascii_lowercase().contains("steamclient"))
                        .collect::<Vec<_>>()
                ),
            );
            if let Some(c) = exit_seen {
                st.log(
                    "watch.log",
                    &format!("the game exited by itself with code 0x{c:X}"),
                );
            }
            sys::kill(pid);
            st.phase = "aborted".into();
            st.note = "P-module: the instance's steamclient64.dll was not loaded in the game; the game was killed".into();
            st.save()?;
            return Err(st.note.clone());
        }
        st.log(
            "watch.log",
            "P-module ok: emulator loaded from the instance folder, ActiveProcess restored",
        );
        st.save()?;
        // The next instance must not start before this one's mod has released the shared names (mutex, dearxan section).
        let t3 = Instant::now();
        let modlog = dir.join("mod").join("nrpvp.log");
        loop {
            if fs::read_to_string(&modlog)
                .is_ok_and(|t| t.contains("multi_instance: ready for another instance"))
            {
                st.log("watch.log", "multi_instance: ready for another instance");
                break;
            }
            if t3.elapsed() > Duration::from_secs(120) || !sys::alive(pid) {
                st.log("watch.log", "multi_instance: no ready line within 120 s");
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        // Queue on: the mod's queue worker must log the emulated Steam id (fail closed: any other id, or none, kills the game).
        if queue_on(st) {
            let t4 = Instant::now();
            let verdict = loop {
                match check_identity(st) {
                    Ok(id) => break Ok(id),
                    Err(e) if e.contains("not the emulated") => break Err(e),
                    Err(e) if t4.elapsed() > Duration::from_secs(180) || !sys::alive(pid) => {
                        break Err(e)
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(500)),
                }
            };
            match verdict {
                Ok(id) => st.log(
                    "watch.log",
                    &format!(
                        "identity ok: the mod's queue worker runs as the emulated Steam id {id}"
                    ),
                ),
                Err(e) => {
                    sys::kill(pid);
                    st.phase = "aborted".into();
                    st.note = format!("identity check: {e}; the game was killed");
                    st.log("watch.log", &st.note);
                    st.save()?;
                    return Err(st.note.clone());
                }
            }
        }
    }

    // L1: me3's attach config.
    let expected =
        profile::paths(&fs::read_to_string(dir.join("profile.me3.toml")).unwrap_or_default());
    let t1 = Instant::now();
    loop {
        let text = fs::read_to_string(dir.join("me3.log")).unwrap_or_default();
        if let Some(a) = me3log::attach(&text) {
            if let Err(why) = me3log::verify(&a, &expected) {
                sys::kill(pid);
                st.phase = "aborted".into();
                st.note = format!("L1 safety: {why}; the game was killed");
                st.save()?;
                return Err(st.note.clone());
            }
            st.log(
                "watch.log",
                "L1 ok: me3 attach config start_online = false, modules as staged",
            );
            break;
        }
        if t1.elapsed() > Duration::from_secs(30) {
            sys::kill(pid);
            st.phase = "aborted".into();
            st.note =
                "L1 safety: me3 printed no attach config within 30 s; the game was killed".into();
            st.save()?;
            return Err(st.note.clone());
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    // Watchdog: a detached `nro watch` that outlives this command.
    // From a copy in the run folder, so the watchdog never locks the nro.exe a rebuild wants to replace.
    // The CLI binary does the watching (`nro watch`); the MCP server finds it next to itself.
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let me = me.with_file_name("nro.exe");
    if !me.is_file() {
        return Err(format!(
            "{} not found (build the whole workspace)",
            me.display()
        ));
    }
    let copy = dir.join("nro-watch.exe");
    let me = if fs::copy(&me, &copy).is_ok() {
        copy
    } else {
        me
    };
    let w = Command::new(me)
        .args(["watch", &st.dir])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("could not start the watchdog: {e}"))?;
    st.watch_pid = Some(w.id());
    st.phase = "running".into();
    st.save()?;
    Ok(r)
}

/// Wait until the mod's console answers (the DLL is loaded and its pipe is up).
pub fn wait_console(st: &State, timeout: Duration) -> Result<(), String> {
    let pid = st.game_pid.ok_or("no game")?;
    let t = Instant::now();
    while t.elapsed() < timeout {
        if !sys::alive(pid) {
            return Err("the game exited".into());
        }
        if console::exists(pid) && console::ask(pid, "whoami", Duration::from_secs(2)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!(
        "the console did not answer within {} s",
        timeout.as_secs()
    ))
}

/// Send a console line, recording it in console.log.
pub fn cmd(st: &State, line: &str) -> Result<String, String> {
    let pid = st.live_pid().ok_or("the game is not running")?;
    if is_queue_line(line) {
        // Only an emulated instance whose mod runs as the emulated id may queue; a real-Steam session never does.
        let why = match &st.emu {
            None => Err(
                "a real-Steam session never queues (nro refuses mm_queue_* / mm_join)".to_string(),
            ),
            Some(_) => check_identity(st).map(|_| ()),
        };
        if let Err(e) = why {
            st.log("console.log", &format!("> {line}\n!! refused by nro: {e}"));
            return Err(format!("refused by nro: {e}"));
        }
    }
    let r = console::ask(pid, line, Duration::from_secs(10));
    match &r {
        Ok(t) => st.log("console.log", &format!("> {line}\n{t}")),
        Err(e) => st.log("console.log", &format!("> {line}\n!! {e}")),
    }
    r
}

/// A crashed `nro` may have left Steam's ActiveProcess key pointing at an emulator: put the saved values back.
pub fn recover_registry() {
    if let Ok(mut s) = State::current() {
        if let Some(e) = s.emu.as_mut().filter(|e| !e.reg_backup.is_empty()) {
            let _ = crate::emu::restore(e);
            let _ = s.save();
        }
    }
}

pub fn stop(st: &mut State) -> String {
    let mut msg = Vec::new();
    if let Some(e) = st.emu.as_mut() {
        let _ = crate::emu::restore(e);
    }
    if let Some(p) = st.live_pid() {
        msg.push(format!("game pid {p} killed: {}", sys::kill(p)));
        // Wait until the process is really gone: a killed instance still holds dearxan's shared section and the engine mutex until
        // it has torn down, and the next launch right after crashes in me3's host (seen 2026-10-27 after `stop --all`).
        let t = Instant::now();
        while sys::alive(p) && t.elapsed() < Duration::from_secs(30) {
            std::thread::sleep(Duration::from_millis(250));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    // Same pid-reuse guard as `live_pid`: only a process that is still me3.
    if let Some(p) = st.me3_pid.filter(|&p| {
        sys::image_path(p).is_some_and(|i| {
            i.rsplit('\\')
                .next()
                .is_some_and(|n| n.to_ascii_lowercase().starts_with("me3"))
        })
    }) {
        msg.push(format!("me3 pid {p} killed: {}", sys::kill(p)));
    }
    st.phase = "ended".into();
    let _ = st.save();
    msg.join("; ")
}

/// `nro watch <dir>`: every 2 s until the game exits. Kills the game if EasyAntiCheat or the anti-cheat launcher shows up;
/// records every remote TCP endpoint the game uses (L2 measurement); after exit, checks the vanilla saves are unchanged.
pub fn watch(dir: &Path) -> Result<(), String> {
    let mut st = State::load(dir)?;
    let pid = st.game_pid.ok_or("no game pid")?;
    let mut seen = std::collections::BTreeSet::new();
    st.log("watch.log", &format!("watchdog on game pid {pid}"));
    let emu = st.emu.is_some();
    while sys::alive(pid) {
        for p in sys::processes() {
            let n = p.name.to_ascii_lowercase();
            // An emulated instance must never run next to the real Steam client (P6 at launch; here for the whole run).
            if n.contains("easyanticheat")
                || n == "start_protected_game.exe"
                || (emu && n == "steam.exe")
            {
                sys::kill(pid);
                st.phase = "aborted".into();
                st.note = format!(
                    "safety: {} (pid {}) appeared; the game was killed",
                    p.name, p.pid
                );
                st.log("watch.log", &st.note);
                st.save()?;
            }
        }
        for ep in crate::net::tcp_remotes(pid) {
            if seen.insert(ep.clone()) {
                st.log("watch.log", &format!("tcp remote {ep}"));
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    let after = vanilla_saves().unwrap_or_default();
    let same = after == st.vanilla_saves;
    st.log(
        "watch.log",
        &format!("game exited; vanilla saves unchanged: {same}"),
    );
    let mut st = State::load(dir).unwrap_or(st);
    if st.phase == "running" || st.phase == "launching" {
        st.phase = "ended".into();
    }
    if !same {
        st.note = format!("{} | vanilla saves CHANGED", st.note);
    }
    st.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_pid_reads_coloured_me3_log() {
        let coloured = "\x1b[32m INFO\x1b[0m \x1b[1mrun\x1b[0m\x1b[2m:\x1b[0m\x1b[1mattach\x1b[0m\x1b[2m:\x1b[0m \x1b[2mme3_launcher::game\x1b[0m\x1b[2m:\x1b[0m attaching to process \x1b[3mpid\x1b[0m\x1b[2m=\x1b[0m9632\n";
        assert_eq!(attach_pid(coloured), Some(9632));
        assert_eq!(
            attach_pid(" INFO run:attach: me3_launcher::game: attaching to process pid=1234\n"),
            Some(1234)
        );
        assert_eq!(attach_pid("Launcher started\n"), None);
        assert_eq!(attach_pid("attaching to process pid=0\n"), None);
    }

    #[test]
    fn forced_config_keeps_real_steam_off_the_queue() {
        let real = forced_config(false);
        assert_eq!(real.get("mm_queue"), Some(&toml::Value::Boolean(false)));
        assert_eq!(real.get("br_menu"), Some(&toml::Value::Boolean(false)));
        let emu = forced_config(true);
        assert!(!emu.contains_key("mm_queue"));
        assert_eq!(
            emu.get("mm_join_auto").and_then(|v| v.as_str()),
            Some("invite")
        );
    }

    #[test]
    fn queue_lines() {
        assert!(is_queue_line("mm_queue_solo"));
        assert!(is_queue_line(" mm_queue_duo"));
        assert!(is_queue_line("mm_join invite"));
        assert!(!is_queue_line("mm_status"));
        assert!(!is_queue_line("mm_leave"));
        assert!(!is_queue_line("log 300"));
    }
}
