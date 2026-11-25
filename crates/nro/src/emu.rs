//! Emulated-Steam instances (docs: .claude/orchestrator/MULTI-INSTANCE.md). One instance = its own game folder on the game's drive
//! (hard links for files, junctions for folders, a copy of `SeamlessCoop\`), the gbe_fork `steamclient64.dll` + `steam_settings\` of the
//! Nucleus handler, and the global `HKCU\Software\Valve\Steam\ActiveProcess` key pointed at it while the game starts (what Nucleus'
//! `steamclient_loader_x64.bat` does). Binaries are copied from the user's Nucleus install and never committed.

use nro_core::pins::Pins;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sys;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const REG_KEY: &str = r"HKCU\Software\Valve\Steam\ActiveProcess";
/// SHA-256 of the handler's gbe_fork steamclient64.dll (S0, 2026-10-26). A different file is refused.
pub const STEAMCLIENT64_SHA256: &str =
    "A3D78CDA6C25D831304AC05040DDBB46D60F6CEB939BC621685F4C4BC407524E";
const DEFAULT_SRC: &str = r"D:\Programs\Nucleus\handlers\ELDEN RING NIGHTREIGN";
/// The emulated Steam ids (index = instance number - 1): the three Nucleus ids that already had saves in %APPDATA%\Nightreign, and
/// (ARENA-5, a 2v2 on one PC) a fourth whose save is made once by playing the new-game start on instance 4.
pub const STEAM_IDS: [u64; 4] = [
    76561199023125438,
    76561199023125439,
    76561199023125440,
    76561199023125441,
];

/// What a run remembers about its emulated instance (`session.json`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmuInfo {
    pub n: u32,
    pub steamid: u64,
    /// The instance's game folder (holds `nightreign.exe`, `steamclient64.dll`, `steam_settings\`).
    pub game_dir: String,
    /// `ActiveProcess` values saved before the launch and not yet restored: (name, type, data).
    pub reg_backup: Vec<(String, String, String)>,
}

impl EmuInfo {
    pub fn exe(&self) -> PathBuf {
        Path::new(&self.game_dir).join("nightreign.exe")
    }
}

pub fn src_dir() -> PathBuf {
    std::env::var_os("NRO_EMU_SRC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SRC))
}

fn instances_root(pins: &Pins) -> Result<PathBuf, String> {
    let g = Path::new(&pins.game_dir);
    let drive = g
        .components()
        .next()
        .ok_or("game_dir has no drive")?
        .as_os_str();
    Ok(PathBuf::from(drive)
        .join(std::path::MAIN_SEPARATOR_STR)
        .join("nro-instances"))
}

fn copy_tree(from: &Path, to: &Path, skip: &[&str]) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for e in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        if skip.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let t = to.join(&name);
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&e.path(), &t, &[])?;
        } else {
            fs::copy(e.path(), &t).map_err(|er| format!("copy {}: {er}", e.path().display()))?;
        }
    }
    Ok(())
}

fn junction(link: &Path, target: &Path) -> Result<(), String> {
    let o = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(format!(
            "mklink /J {}: {}",
            link.display(),
            String::from_utf8_lossy(&o.stdout)
        ))
    }
}

/// The text of the staged `steam_settings\configs.user.ini`.
pub fn user_ini(n: u32, steamid: u64) -> String {
    format!("[user::general]\naccount_name=nro{n}\naccount_steamid={steamid}\nlanguage=english\nip_country=US\n")
}

/// The staged `configs.main.ini`: LAN-only on (`disable_lan_only=0`, checked by preflight P6b), the rest as the emulator's defaults.
pub fn main_ini(n: u32) -> String {
    // One emulator port per instance (a shared 47584 is a suspect for the S2 crash of the second instance).
    format!("[main::general]\nnew_app_ticket=1\ngc_token=1\n[main::connectivity]\ndisable_lan_only=0\nlisten_port={}\noffline=0\n", 47583 + n)
}

/// Build instance `n`: returns its [`EmuInfo`] (registry backup empty). The folder of instance `n` is the SAME for every run
/// (`nro-instances\i<N>\game` on the game's drive), so Windows Firewall asks once per instance, not once per run; staging only
/// refreshes what changed.
pub fn stage_instance(pins: &Pins, n: u32) -> Result<EmuInfo, String> {
    let steamid = *STEAM_IDS
        .get(n as usize - 1)
        .ok_or("at most 4 emulated instances")?;
    let src = src_dir();
    let sc64 = src.join("steamclient64.dll");
    let got = sys::sha256(&sc64).ok_or_else(|| format!("{} not readable", sc64.display()))?;
    if !got.eq_ignore_ascii_case(STEAMCLIENT64_SHA256) {
        return Err(format!(
            "{} is not the known gbe_fork build ({got})",
            sc64.display()
        ));
    }
    let game = PathBuf::from(&pins.game_dir);
    let dir = instances_root(pins)?.join(format!("i{n}")).join("game");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in fs::read_dir(&game).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        let to = dir.join(&name);
        if name.eq_ignore_ascii_case("SeamlessCoop") {
            // Own nrsc_settings.ini per instance (SC reads it next to nrsc.dll); no crash reports.
            copy_tree(&e.path(), &to, &["crashdumps", "crashpad"])?;
        } else if name.eq_ignore_ascii_case("NightreignPvP") {
            continue;
        } else if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            if !to.exists() {
                junction(&to, &e.path())?;
            }
        } else if name.eq_ignore_ascii_case("nightreign.exe") {
            // A real copy of the exe (Nucleus does the same), refreshed only when the game's file changed.
            let src_meta = e.metadata().map_err(|e| e.to_string())?;
            let stale = match fs::metadata(&to) {
                Ok(m) => m.len() != src_meta.len(),
                Err(_) => true,
            };
            if stale {
                fs::copy(e.path(), &to)
                    .map_err(|er| format!("copy {}: {er}", e.path().display()))?;
            }
        } else {
            // Every other file is a hard link, re-made so that a game update (a new file behind the same name) is followed.
            if to.exists() {
                fs::remove_file(&to).map_err(|er| format!("remove {}: {er}", to.display()))?;
            }
            fs::hard_link(e.path(), &to)
                .map_err(|er| format!("hard link {}: {er}", e.path().display()))?;
        }
    }
    fs::copy(&sc64, dir.join("steamclient64.dll")).map_err(|e| e.to_string())?;
    let ss = dir.join("steam_settings");
    fs::create_dir_all(&ss).map_err(|e| e.to_string())?;
    for f in [
        "configs.app.ini",
        "steam_interfaces.txt",
        "auto_accept_invite.txt",
        "auto_send_invite.txt",
    ] {
        let from = src.join("steam_settings").join(f);
        if from.is_file() {
            fs::copy(&from, ss.join(f)).map_err(|e| e.to_string())?;
        }
    }
    fs::write(ss.join("configs.user.ini"), user_ini(n, steamid)).map_err(|e| e.to_string())?;
    fs::write(ss.join("configs.main.ini"), main_ini(n)).map_err(|e| e.to_string())?;
    fs::write(ss.join("steam_appid.txt"), "2622380").map_err(|e| e.to_string())?;
    fs::write(dir.join("steam_appid.txt"), "2622380").map_err(|e| e.to_string())?;
    Ok(EmuInfo {
        n,
        steamid,
        game_dir: dir.display().to_string(),
        reg_backup: Vec::new(),
    })
}

fn reg(args: &[&str]) -> Result<String, String> {
    let o = Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).to_string())
    }
}

/// Parse `reg query` output lines `    name    REG_SZ    data`.
pub fn parse_reg(out: &str) -> Vec<(String, String, String)> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next()?;
            let ty = it.next()?;
            if !ty.starts_with("REG_") {
                return None;
            }
            let data = l.split_once(ty)?.1.trim().to_string();
            Some((name.to_string(), ty.to_string(), data))
        })
        .collect()
}

const KEEP: [&str; 3] = ["pid", "SteamClientDll", "SteamClientDll64"];

/// Remember the three values, then point the key at this instance's emulator (pid = `pid`).
pub fn redirect(info: &mut EmuInfo, pid: u32) -> Result<(), String> {
    let cur = parse_reg(&reg(&["query", REG_KEY]).unwrap_or_default());
    info.reg_backup = cur
        .into_iter()
        .filter(|(n, _, _)| KEEP.contains(&n.as_str()))
        .collect();
    let d = Path::new(&info.game_dir);
    reg(&[
        "add",
        REG_KEY,
        "/v",
        "SteamClientDll64",
        "/t",
        "REG_SZ",
        "/d",
        &d.join("steamclient64.dll").display().to_string(),
        "/f",
    ])?;
    reg(&[
        "add",
        REG_KEY,
        "/v",
        "pid",
        "/t",
        "REG_DWORD",
        "/d",
        &pid.to_string(),
        "/f",
    ])?;
    Ok(())
}

/// Put the saved values back (no-op when nothing is pending).
pub fn restore(info: &mut EmuInfo) -> Result<(), String> {
    for (n, t, d) in std::mem::take(&mut info.reg_backup) {
        let d = if t == "REG_DWORD" {
            u32::from_str_radix(d.trim_start_matches("0x"), 16)
                .map(|v| v.to_string())
                .unwrap_or(d)
        } else {
            d
        };
        reg(&["add", REG_KEY, "/v", &n, "/t", &t, "/d", &d, "/f"])?;
    }
    Ok(())
}

/// P-module: the module paths of a process contain `steamclient64.dll` from `game_dir` (identity proof; `whoami` has no Steam id).
pub fn module_ok(modules: &[String], game_dir: &str) -> bool {
    let want = Path::new(game_dir)
        .join("steamclient64.dll")
        .display()
        .to_string()
        .to_ascii_lowercase();
    modules.iter().any(|m| m.to_ascii_lowercase() == want)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reg_output() {
        let out = "\nHKEY_CURRENT_USER\\x\n    pid    REG_DWORD    0x3768\n    SteamClientDll64    REG_SZ    C:\\Program Files (x86)\\Steam\\steamclient64.dll\n";
        let v = parse_reg(out);
        assert_eq!(v[0], ("pid".into(), "REG_DWORD".into(), "0x3768".into()));
        assert_eq!(v[1].2, r"C:\Program Files (x86)\Steam\steamclient64.dll");
    }

    #[test]
    fn module_check_needs_the_instance_copy() {
        let m = vec![r"C:\Program Files (x86)\Steam\steamclient64.dll".to_string()];
        assert!(!module_ok(&m, r"Z:\nro-instances\r\i1\game"));
        let m = vec![r"Z:\nro-instances\r\i1\game\SteamClient64.dll".to_string()];
        assert!(module_ok(&m, r"Z:\nro-instances\r\i1\game"));
    }

    #[test]
    fn ini_texts() {
        assert!(user_ini(1, 76561199023125438).contains("account_steamid=76561199023125438"));
        assert!(main_ini(2).contains("disable_lan_only=0"));
        assert!(main_ini(2).contains("listen_port=47585"));
    }
}
