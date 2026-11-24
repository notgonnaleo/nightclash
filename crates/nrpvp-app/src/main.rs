//! NightreignPvP.exe: the launcher app. A first-run wizard (game folder, Seamless Co-op, the mod), updates from GitHub releases,
//! and Play, which starts the game exactly like nrpvp_launcher.exe (shared code in crates/nrpvp-launcher) and then closes the app so
//! it uses no memory while the game runs. The window is the HTML in ui\ (Tauri 2, WebView2).
#![windows_subsystem = "windows"]

mod auth;
mod game;
mod install;
mod room;
mod save_format;
mod saves;
mod update;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

// The release lookup lives in the launcher crate, shared with the version gate of nrpvp_launcher.exe (LAUNCH-GATE).
use nrpvp_launch::release;
use release::{Manifest, ScRelease};

// ---- settings ---------------------------------------------------------------------------------------------------------------
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    game_dir: Option<PathBuf>,
    /// Cache for the game exe hash: (size, modified secs, SHA-256). The exe is ~120 MB.
    #[serde(default)]
    exe_hash: Option<(u64, u64, String)>,
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("NightreignPvP").join("launcher.json")
}

fn load_settings() -> Settings {
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_settings()
}

fn read_settings() -> Settings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn update_settings(edit: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "Restart the launcher to update settings.")?;
    let mut settings = read_settings();
    edit(&mut settings);
    let p = settings_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &p,
        serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Could not save the launcher settings: {e}"))
}

/// The saved game folder if it is still valid, else a fresh detection (saved for next time).
fn game_dir() -> Option<PathBuf> {
    let s = load_settings();
    if let Some(dir) = s.game_dir.as_deref().filter(|d| game::is_game_dir(d)) {
        return Some(dir.to_path_buf());
    }
    let found = game::detect()?;
    let _ = update_settings(|s| s.game_dir = Some(found.clone()));
    Some(found)
}

fn need_game() -> Result<PathBuf, String> {
    game_dir().ok_or_else(|| "Choose the game folder first.".to_string())
}

fn work_dir() -> Result<PathBuf, String> {
    let d = std::env::temp_dir().join("NightreignPvP-launcher");
    std::fs::create_dir_all(&d).map_err(|e| format!("Could not create {}: {e}", d.display()))?;
    Ok(d)
}

// ---- progress ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Serialize)]
struct Progress<'a> {
    label: &'a str,
    done: u64,
    total: u64,
}

fn reporter<'a>(app: &'a AppHandle, label: &'a str) -> impl FnMut(u64, u64) + 'a {
    let mut last_pct = u64::MAX;
    move |done, total| {
        let pct = (done * 200).checked_div(total).unwrap_or(0);
        if pct != last_pct || done == total {
            last_pct = pct;
            let _ = app.emit("progress", Progress { label, done, total });
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("Internal error: {e}"))?
}

// ---- commands ---------------------------------------------------------------------------------------------------------------

#[derive(Serialize)]
struct Status {
    launcher_version: &'static str,
    /// SHA-256 of this exe, to tell the player when the release carries a newer launcher.
    launcher_sha256: Option<String>,
    game_dir: Option<String>,
    sc_installed: bool,
    sc_sha256: Option<String>,
    sc_launcher: bool,
    #[serde(rename = "mod")]
    mod_state: Option<install::ModState>,
    game_running: bool,
    /// Files of other tools in the game folder that break matches (`game::foreign_files`).
    foreign: Vec<String>,
    /// RW-6: the private room code in the installed `nrpvp.toml` ("" = public matchmaking).
    private_code: String,
    player: saves::Status,
    auto_update: bool,
}

#[tauri::command]
async fn status() -> Result<Status, String> {
    blocking(|| {
        let game = game_dir();
        let nrsc = game
            .as_ref()
            .map(|g| g.join("SeamlessCoop").join("nrsc.dll"));
        let sc_installed = nrsc.as_ref().is_some_and(|p| p.is_file());
        Ok(Status {
            launcher_version: update::VERSION,
            launcher_sha256: std::env::current_exe()
                .ok()
                .and_then(|p| release::sha256_file(&p).ok()),
            game_dir: game.as_ref().map(|g| g.display().to_string()),
            sc_installed,
            sc_sha256: nrsc
                .filter(|_| sc_installed)
                .and_then(|p| release::sha256_file(&p).ok()),
            sc_launcher: game
                .as_ref()
                .is_some_and(|g| g.join("nrsc_launcher.exe").is_file()),
            mod_state: game.as_deref().map(install::mod_state),
            game_running: game::is_running(),
            foreign: game.as_deref().map(game::foreign_files).unwrap_or_default(),
            private_code: game.as_deref().map(install::room_code).unwrap_or_default(),
            player: saves::status(game.as_deref(), saves::detect_account()),
            auto_update: !std::env::args().any(|a| a == "--skip-auto-update"),
        })
    })
    .await
}

/// SHA-256 of nightreign.exe (cached while size and date stay the same), to tell a game update apart from the pinned build.
#[tauri::command]
async fn game_exe_sha256() -> Result<Option<String>, String> {
    blocking(|| {
        let Some(game) = game_dir() else {
            return Ok(None);
        };
        let exe = game.join("nightreign.exe");
        let meta = std::fs::metadata(&exe).map_err(|e| e.to_string())?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let s = load_settings();
        if let Some((size, m, h)) = &s.exe_hash {
            if *size == meta.len() && *m == mtime {
                return Ok(Some(h.clone()));
            }
        }
        let h = release::sha256_file(&exe)?;
        let _ = update_settings(|s| {
            if s.game_dir.as_deref() == Some(game.as_path()) {
                s.exe_hash = Some((meta.len(), mtime, h.clone()));
            }
        });
        Ok(Some(h))
    })
    .await
}

#[tauri::command]
async fn pick_game_dir() -> Result<Option<String>, String> {
    let picked = blocking(|| {
        Ok(rfd::FileDialog::new()
            .set_title("Choose the ELDEN RING NIGHTREIGN folder (the one with nightreign.exe)")
            .pick_folder())
    })
    .await?;
    match picked {
        None => Ok(None),
        Some(p) => set_game_dir(p.display().to_string()).map(Some),
    }
}

#[tauri::command]
fn set_game_dir(path: String) -> Result<String, String> {
    let dir = game::normalize(Path::new(&path))
        .ok_or("nightreign.exe is not in that folder. Pick the game's \"Game\" folder: in Steam, right-click ELDEN RING NIGHTREIGN > Manage > Browse local files.")?;
    update_settings(|s| {
        s.game_dir = Some(dir.clone());
        s.exe_hash = None;
    })?;
    Ok(dir.display().to_string())
}

#[tauri::command]
async fn latest_release() -> Result<Manifest, String> {
    blocking(release::latest).await
}

#[tauri::command]
async fn install_mod(app: AppHandle) -> Result<String, String> {
    blocking(move || {
        let game = need_game()?;
        if game::is_running() {
            return Err("Close the game first, then install.".into());
        }
        let m = release::latest()?;
        let zip = work_dir()?.join(&m.zip.name);
        release::fetch_asset(&m, &m.zip, &zip, reporter(&app, "Downloading Nightclash"))?;
        install::install_mod(&game, &zip, &m, reporter(&app, "Installing Nightclash"))?;
        let _ = std::fs::remove_file(&zip);
        Ok(m.version)
    })
    .await
}

#[tauri::command]
async fn sc_github() -> Result<Option<ScRelease>, String> {
    blocking(release::sc_github_release).await
}

#[tauri::command]
async fn install_sc_github(app: AppHandle, rel: ScRelease) -> Result<(), String> {
    blocking(move || {
        let game = need_game()?;
        if game::is_running() {
            return Err("Close the game first, then install.".into());
        }
        let zip = work_dir()?.join("SeamlessCoop.zip");
        release::copy_hashed(
            release::open_url(&rel.url)?,
            &zip,
            rel.size,
            reporter(&app, "Downloading Seamless Co-op"),
        )?;
        install::install_sc(&game, &zip, reporter(&app, "Installing Seamless Co-op"))?;
        let _ = std::fs::remove_file(&zip);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn find_sc_zip() -> Result<Option<String>, String> {
    blocking(|| Ok(install::find_sc_zip_in_downloads().map(|p| p.display().to_string()))).await
}

#[tauri::command]
async fn pick_sc_zip() -> Result<Option<String>, String> {
    blocking(|| {
        let mut dlg = rfd::FileDialog::new()
            .set_title("Choose the Seamless Co-op zip you downloaded")
            .add_filter("Zip", &["zip"]);
        if let Some(home) = std::env::var_os("USERPROFILE") {
            dlg = dlg.set_directory(PathBuf::from(home).join("Downloads"));
        }
        Ok(dlg.pick_file().map(|p| p.display().to_string()))
    })
    .await
}

#[tauri::command]
async fn install_sc_zip(app: AppHandle, path: String) -> Result<(), String> {
    blocking(move || {
        let game = need_game()?;
        if game::is_running() {
            return Err("Close the game first, then install.".into());
        }
        install::install_sc(
            &game,
            Path::new(&path),
            reporter(&app, "Installing Seamless Co-op"),
        )
    })
    .await
}

/// RW-6: set the private room code ("" = public matchmaking); answers the code as saved (normalised).
#[tauri::command]
fn set_private_code(code: String) -> Result<String, String> {
    let code = room::normalize(&code)?;
    let game = need_game()?;
    if game::is_running() {
        return Err("Close the game first: it reads the code when it starts.".into());
    }
    install::set_room_code(&game, &code)?;
    Ok(code)
}

/// Opens a fixed place: never a URL or path from the page.
#[tauri::command]
fn open(target: String) -> Result<(), String> {
    let what = match target.as_str() {
        "nexus" => release::SC_NEXUS_URL.to_string(),
        "releases" => release::releases_page(),
        "game" => need_game()?.display().to_string(),
        "logs" => need_game()?
            .join(nrpvp_launch::MOD_DIR)
            .display()
            .to_string(),
        _ => return Err(format!("unknown target {target}")),
    };
    let (program, args) = opener(&what);
    Command::new(program)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// How a place is opened (pure): a web page in the default browser, a folder in Explorer. `explorer <url>` does not open the browser
/// for every URL: with the Nexus link (`?tab=files`) it opened a File Explorer window instead (Leonardo, 2026-11-10).
fn opener(what: &str) -> (&'static str, Vec<String>) {
    if what.starts_with("https://") || what.starts_with("http://") {
        (
            "rundll32.exe",
            vec!["url.dll,FileProtocolHandler".into(), what.into()],
        )
    } else {
        ("explorer", vec![what.into()])
    }
}

#[cfg(test)]
mod open_tests {
    use super::*;

    #[test]
    fn a_web_page_goes_to_the_browser_and_a_folder_to_explorer() {
        let (p, a) = opener(release::SC_NEXUS_URL);
        assert_eq!(p, "rundll32.exe");
        assert_eq!(
            a,
            [
                "url.dll,FileProtocolHandler".to_string(),
                release::SC_NEXUS_URL.to_string()
            ]
        );
        let (p, a) = opener(r"Z:\Game\NightreignPvP");
        assert_eq!(p, "explorer");
        assert_eq!(a, [r"Z:\Game\NightreignPvP".to_string()]);
    }
}

#[tauri::command]
async fn update_launcher(app: AppHandle) -> Result<bool, String> {
    let handle = app.clone();
    let started = blocking(move || {
        if game::is_running() {
            return Err("Close the game before updating the launcher.".into());
        }
        update::prepare(reporter(&handle, "Updating the launcher"))
    })
    .await?;
    if started {
        app.exit(0);
    }
    Ok(started)
}

#[tauri::command]
async fn setup_player() -> Result<(), String> {
    blocking(move || {
        let steam_id = saves::current_account()?;
        saves::setup(&need_game()?, &steam_id)?;
        Ok(())
    })
    .await
}

#[tauri::command]
async fn rename_player(slot: usize, name: String, steam_id: String) -> Result<(), String> {
    blocking(move || {
        let id = saves::require_account(&steam_id)?;
        saves::rename(&need_game()?, &id, slot, &name)
    })
    .await
}

#[tauri::command]
async fn replace_starter_save(steam_id: String) -> Result<(), String> {
    blocking(move || saves::reset(&need_game()?, &steam_id)).await
}

#[tauri::command]
async fn import_steam_save(steam_id: String) -> Result<bool, String> {
    blocking(move || saves::import_steam(&need_game()?, &steam_id)).await
}

/// Starts the game, then closes the launcher. `nrpvp_launch::launch` runs the version gate: an old mod install is refused with the
/// player message (the UI then shows its Update button).
#[tauri::command]
async fn play(app: AppHandle) -> Result<(), String> {
    blocking(|| {
        let game = need_game()?;
        let id = saves::current_account()?;
        saves::setup(&game, &id)?;
        if game::is_running() {
            return Err("The game is already running.".into());
        }
        saves::require_account(&id)?;
        nrpvp_launch::launch(&game).map_err(|e| e.to_string())
    })
    .await?;
    app.exit(0);
    Ok(())
}

fn main() {
    if update::run_helper() {
        return;
    }
    auth::forget_saved_token();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            status,
            game_exe_sha256,
            pick_game_dir,
            set_game_dir,
            latest_release,
            install_mod,
            sc_github,
            install_sc_github,
            find_sc_zip,
            pick_sc_zip,
            install_sc_zip,
            open,
            set_private_code,
            setup_player,
            rename_player,
            replace_starter_save,
            import_steam_save,
            update_launcher,
            play
        ])
        .run(tauri::generate_context!())
        .expect("the launcher window could not start (is Microsoft Edge WebView2 installed?)");
}
