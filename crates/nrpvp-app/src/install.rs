//! Putting files into the game folder: the mod zip and the Seamless Co-op zip. Both are extracted with our own path check (no
//! entry may leave the game folder), and the names are normalised first: Windows PowerShell 5.1's `Compress-Archive` writes `\`.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::release::Manifest;
use nrpvp_launch::MOD_DIR;
// INSTALLED_MANIFEST: the installed release's manifest, written into the mod folder after an install; the version gate reads it too.
use nrpvp_launch::version::{Installed, INSTALLED_MANIFEST};

/// Folders inside the mod folder that belong to the release and are replaced as a whole (no stale files from an old version).
const REPLACED_DIRS: [&str; 2] = ["gamedata", "me3"];
const SC_SETTINGS: &str = "SeamlessCoop/nrsc_settings.ini";

/// A zip entry name as a relative path, or `None` when it is a folder entry or would leave the target folder.
pub fn safe_rel(name: &str) -> Option<PathBuf> {
    let norm = name.replace('\\', "/");
    if norm.ends_with('/') {
        return None;
    }
    let p = PathBuf::from(norm.trim_start_matches('/'));
    let ok = p.components().all(|c| matches!(c, Component::Normal(_)));
    (ok && p.components().next().is_some()).then_some(p)
}

fn norm_name(name: &str) -> String {
    name.replace('\\', "/")
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<fs::File>, String> {
    let f = fs::File::open(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    zip::ZipArchive::new(f).map_err(|e| format!("{} is not a valid zip: {e}", path.display()))
}

/// Extracts the entries for which `pick(normalised name)` gives a target path relative to `root`.
fn extract(
    zip_path: &Path,
    root: &Path,
    pick: impl Fn(&str) -> Option<PathBuf>,
    mut progress: impl FnMut(u64, u64),
) -> Result<usize, String> {
    let mut archive = open_zip(zip_path)?;
    let chosen: Vec<(usize, PathBuf)> = (0..archive.len())
        .filter_map(|i| {
            let name = norm_name(archive.by_index_raw(i).ok()?.name());
            pick(&name).map(|rel| (i, rel))
        })
        .collect();
    let total = chosen.len() as u64;
    for (n, (i, rel)) in chosen.iter().enumerate() {
        let mut entry = archive.by_index(*i).map_err(|e| e.to_string())?;
        let dest = root.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut data)
            .map_err(|e| format!("The zip is damaged ({}): {e}", rel.display()))?;
        fs::write(&dest, &data).map_err(|e| {
            format!(
                "Could not write {}: {e}\n\nIs the game still running? Close it and try again.",
                dest.display()
            )
        })?;
        progress(n as u64 + 1, total);
    }
    Ok(chosen.len())
}

/// The mod's settings file inside [`MOD_DIR`].
const TOML: &str = "nrpvp.toml";

/// RW-6: the private room code in the installed `nrpvp.toml` ("" = public, or no file).
pub fn room_code(game: &Path) -> String {
    fs::read_to_string(game.join(MOD_DIR).join(TOML))
        .map(|t| crate::room::read(&t))
        .unwrap_or_default()
}

/// RW-6: write `code` (normalised by the caller; "" = public) into the installed `nrpvp.toml`.
pub fn set_room_code(game: &Path, code: &str) -> Result<(), String> {
    let path = game.join(MOD_DIR).join(TOML);
    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "Could not read {}: {e}\n\nInstall Nightclash first.",
            path.display()
        )
    })?;
    fs::write(&path, crate::room::write(&text, code))
        .map_err(|e| format!("Could not save {}: {e}", path.display()))
}

/// Installs the mod zip (`nrpvp_launcher.exe` + `NightreignPvP\...`) into `game` and records `m` as the installed release.
/// The player's `nrpvp.toml` is replaced by the release's (the release decides match size); the old one is kept as `.bak`, and the
/// player's private room code is carried over.
pub fn install_mod(
    game: &Path,
    zip_path: &Path,
    m: &Manifest,
    progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let prefix = format!("{MOD_DIR}/");
    {
        let archive = open_zip(zip_path)?;
        let names: Vec<String> = archive.file_names().map(norm_name).collect();
        let has = |n: &str| names.iter().any(|x| x.eq_ignore_ascii_case(n));
        if !has(&format!("{prefix}nrpvp.dll")) || !has("nrpvp_launcher.exe") {
            return Err(
                "The downloaded zip is not a Nightclash release (nrpvp.dll is missing).".into(),
            );
        }
    }
    let mod_dir = game.join(MOD_DIR);
    let toml = mod_dir.join(TOML);
    // RW-6: the player's private room code outlives the update (the rest of the file is the release's).
    let room_code = fs::read_to_string(&toml)
        .map(|t| crate::room::read(&t))
        .unwrap_or_default();
    if toml.is_file() {
        let _ = fs::copy(&toml, mod_dir.join("nrpvp.toml.bak"));
    }
    for d in REPLACED_DIRS {
        let p = mod_dir.join(d);
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| {
                format!("Could not replace {}: {e}\n\nIs the game still running? Close it and try again.", p.display())
            })?;
        }
    }
    extract(
        zip_path,
        game,
        |name| {
            let lower = name.to_ascii_lowercase();
            let wanted =
                lower == "nrpvp_launcher.exe" || lower.starts_with(&prefix.to_ascii_lowercase());
            if wanted {
                safe_rel(name)
            } else {
                None
            }
        },
        progress,
    )?;
    if !room_code.is_empty() {
        set_room_code(game, &room_code)?;
    }
    let json = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    fs::write(mod_dir.join(INSTALLED_MANIFEST), json)
        .map_err(|e| format!("Could not record the install: {e}"))
}

/// The folder prefix inside a Seamless Co-op zip that maps onto the game folder (the folder holding `SeamlessCoop\nrsc.dll`), or
/// `None` when the zip is not Seamless Co-op for Nightreign. Kept pure for the unit test.
pub fn sc_prefix<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    const DLL: &str = "seamlesscoop/nrsc.dll";
    names.into_iter().find_map(|n| {
        let lower = norm_name(n).to_ascii_lowercase();
        let base = lower.strip_suffix(DLL)?;
        (base.is_empty() || base.ends_with('/')).then(|| norm_name(n)[..base.len()].to_string())
    })
}

/// True when the zip at `path` is Seamless Co-op for Nightreign.
pub fn is_sc_zip(path: &Path) -> bool {
    open_zip(path)
        .map(|a| sc_prefix(a.file_names()).is_some())
        .unwrap_or(false)
}

/// Installs a Seamless Co-op zip into `game`. An existing `nrsc_settings.ini` (the player's settings) is kept.
pub fn install_sc(
    game: &Path,
    zip_path: &Path,
    progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let prefix = {
        let archive = open_zip(zip_path)?;
        sc_prefix(archive.file_names()).ok_or(
            "This zip is not Seamless Co-op for ELDEN RING NIGHTREIGN (there is no SeamlessCoop\\nrsc.dll inside).",
        )?
    };
    let keep_settings = game.join(SC_SETTINGS).is_file();
    extract(
        zip_path,
        game,
        |name| {
            let rest = name
                .get(prefix.len()..)
                .filter(|_| name.starts_with(&prefix))?;
            if keep_settings && rest.eq_ignore_ascii_case(SC_SETTINGS) {
                return None;
            }
            safe_rel(rest)
        },
        progress,
    )
    .map(|_| ())
}

/// The newest Seamless Co-op zip in the player's Downloads folder (any name; the content decides).
pub fn find_sc_zip_in_downloads() -> Option<PathBuf> {
    let downloads = PathBuf::from(std::env::var_os("USERPROFILE")?).join("Downloads");
    let mut zips: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(downloads)
        .ok()?
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("zip"))
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    zips.sort_by_key(|z| std::cmp::Reverse(z.0));
    zips.into_iter()
        .take(40)
        .map(|(_, p)| p)
        .find(|p| is_sc_zip(p))
}

#[derive(Debug, Clone, Serialize)]
pub struct ModState {
    /// The installed release's version (`VERSION.txt`, else the app's `release.json`), the same one the version gate reads.
    pub version: Option<String>,
    /// A zip built on a developer PC (`version local`): never auto-updated, never blocked.
    pub local_build: bool,
    /// `None` = every file the game start needs is there; else what is missing, in player words.
    pub problem: Option<String>,
    /// The mod folder exists (installed by hand or by the launcher).
    pub present: bool,
}

pub fn mod_state(game: &Path) -> ModState {
    let installed = nrpvp_launch::version::installed(game);
    ModState {
        version: match &installed {
            Installed::Release(v) => Some(v.to_string()),
            _ => None,
        },
        local_build: installed == Installed::Local,
        problem: nrpvp_launch::check(game),
        present: game.join(MOD_DIR).join("nrpvp.dll").is_file(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_rel_rejects_escapes_and_folders() {
        assert_eq!(
            safe_rel(r"NightreignPvP\me3\me3.exe"),
            Some(PathBuf::from("NightreignPvP/me3/me3.exe"))
        );
        assert_eq!(safe_rel("NightreignPvP/"), None);
        assert_eq!(safe_rel("../evil.dll"), None);
        assert_eq!(safe_rel(r"a\..\..\evil.dll"), None);
        assert_eq!(safe_rel("C:/Windows/evil.dll"), None);
        assert_eq!(safe_rel("/abs.dll"), Some(PathBuf::from("abs.dll")));
        assert_eq!(safe_rel(""), None);
    }

    #[test]
    fn sc_prefix_finds_the_game_root_inside_the_zip() {
        assert_eq!(
            sc_prefix(["nrsc_launcher.exe", "SeamlessCoop/nrsc.dll"]),
            Some(String::new())
        );
        assert_eq!(
            sc_prefix(["Seamless Co-op/Game/SeamlessCoop/nrsc.dll"]),
            Some("Seamless Co-op/Game/".to_string())
        );
        assert_eq!(
            sc_prefix([r"Game\SeamlessCoop\nrsc.dll"]),
            Some("Game/".to_string())
        );
        assert_eq!(sc_prefix(["ersc.dll", "SeamlessCoop/ersc.dll"]), None);
        assert_eq!(sc_prefix(["NotSeamlessCoop/nrsc.dll"]), None);
    }

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nrpvp-app-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, data) in entries {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut w, data).unwrap();
        }
        w.finish().unwrap();
    }

    fn manifest() -> Manifest {
        crate::release::parse(
            r#"{"schema":1,"version":"9.9.9","tag":"v9.9.9","published":"2026-11-06",
                "zip":{"name":"z.zip","sha256":"","size":1},"game":{"version":"1","exe_sha256":""},
                "seamless_coop":{"version":"1.1.3","nrsc_sha256":""}}"#,
        )
        .unwrap()
    }

    #[test]
    fn install_mod_takes_backslash_names_replaces_old_folders_and_ignores_the_rest() {
        let dir = temp("mod");
        let game = dir.join("Game");
        fs::create_dir_all(game.join("NightreignPvP").join("me3")).unwrap();
        fs::write(
            game.join("NightreignPvP").join("me3").join("stale.dll"),
            b"old",
        )
        .unwrap();
        fs::write(
            game.join("NightreignPvP").join("nrpvp.toml"),
            b"private_code = \"x\"",
        )
        .unwrap();
        fs::create_dir_all(game.join("NightreignPvP").join("logs")).unwrap();
        fs::write(
            game.join("NightreignPvP").join("logs").join("a.log"),
            b"keep",
        )
        .unwrap();
        let zip = dir.join("m.zip");
        write_zip(
            &zip,
            &[
                ("nrpvp_launcher.exe", b"L"),
                (r"NightreignPvP\nrpvp.dll", b"D"),
                (r"NightreignPvP\nrpvp.toml", b"team_size = 1"),
                (r"NightreignPvP\me3\me3.exe", b"M"),
                ("HOW TO INSTALL.txt", b"t"),
                (r"NightreignPvP\..\..\escape.txt", b"bad"),
            ],
        );
        let mut calls = 0;
        install_mod(&game, &zip, &manifest(), |_, _| calls += 1).unwrap();
        let m = game.join("NightreignPvP");
        assert_eq!(fs::read(game.join("nrpvp_launcher.exe")).unwrap(), b"L");
        assert_eq!(fs::read(m.join("me3").join("me3.exe")).unwrap(), b"M");
        assert!(
            !m.join("me3").join("stale.dll").exists(),
            "me3 folder replaced as a whole"
        );
        assert!(m.join("logs").join("a.log").exists(), "logs kept");
        assert_eq!(
            fs::read(m.join("nrpvp.toml.bak")).unwrap(),
            b"private_code = \"x\""
        );
        assert!(!game.join("HOW TO INSTALL.txt").exists());
        assert!(
            !dir.join("escape.txt").exists() && !dir.parent().unwrap().join("escape.txt").exists()
        );
        assert_eq!(mod_state(&game).version.as_deref(), Some("9.9.9"));
        let new_toml = fs::read_to_string(m.join("nrpvp.toml")).unwrap();
        assert!(
            new_toml.starts_with("team_size = 1\n"),
            "the release's file: {new_toml}"
        );
        assert_eq!(room_code(&game), "x", "the private room code is kept");
        set_room_code(&game, "").unwrap();
        assert_eq!(room_code(&game), "");
        assert_eq!(calls, 4);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_mod_refuses_a_foreign_zip() {
        let dir = temp("foreign");
        let zip = dir.join("x.zip");
        write_zip(&zip, &[("readme.txt", b"x")]);
        assert!(install_mod(&dir, &zip, &manifest(), |_, _| {})
            .unwrap_err()
            .contains("not a Nightclash release"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_sc_maps_the_inner_folder_and_keeps_settings() {
        let dir = temp("sc");
        let game = dir.join("Game");
        fs::create_dir_all(game.join("SeamlessCoop")).unwrap();
        fs::write(game.join("SeamlessCoop").join("nrsc_settings.ini"), b"mine").unwrap();
        let zip = dir.join("Seamless Co-op.zip");
        write_zip(
            &zip,
            &[
                ("Seamless Co-op/nrsc_launcher.exe", b"L"),
                ("Seamless Co-op/SeamlessCoop/nrsc.dll", b"D"),
                ("Seamless Co-op/SeamlessCoop/nrsc_settings.ini", b"theirs"),
                ("Seamless Co-op/SeamlessCoop/locale/english.json", b"{}"),
                ("other/readme.txt", b"x"),
            ],
        );
        assert!(is_sc_zip(&zip));
        install_sc(&game, &zip, |_, _| {}).unwrap();
        assert_eq!(fs::read(game.join("nrsc_launcher.exe")).unwrap(), b"L");
        assert_eq!(
            fs::read(game.join("SeamlessCoop").join("nrsc.dll")).unwrap(),
            b"D"
        );
        assert_eq!(
            fs::read(game.join("SeamlessCoop").join("nrsc_settings.ini")).unwrap(),
            b"mine"
        );
        assert!(game
            .join("SeamlessCoop")
            .join("locale")
            .join("english.json")
            .is_file());
        assert!(!game.join("other").exists() && !game.join("readme.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A real staged release, the way the app installs it (manifest, checked copy, install):
    /// `scripts\publish-release.ps1 -DryRun ...`, then `NRPVP_RELEASE_DIR=dist\release cargo test -p nrpvp-app -- --ignored`.
    #[test]
    #[ignore]
    fn staged_release_installs_into_a_fake_game_folder() {
        assert!(
            std::env::var_os("NRPVP_RELEASE_DIR").is_some(),
            "set NRPVP_RELEASE_DIR"
        );
        let m = crate::release::latest().unwrap();
        let dir = temp("real");
        fs::write(dir.join("nightreign.exe"), b"x").unwrap();
        fs::create_dir_all(dir.join("SeamlessCoop")).unwrap();
        fs::write(dir.join("SeamlessCoop").join("nrsc.dll"), b"x").unwrap();
        let zip = dir.join("download.zip");
        crate::release::fetch_asset(&m, &m.zip, &zip, |_, _| {}).unwrap();
        install_mod(&dir, &zip, &m, |_, _| {}).unwrap();
        assert_eq!(nrpvp_launch::check(&dir), None);
        assert_eq!(mod_state(&dir).version, Some(m.version));
        let _ = fs::remove_dir_all(&dir);
    }
}
