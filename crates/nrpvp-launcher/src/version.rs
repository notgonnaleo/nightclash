//! The version gate (LAUNCH-GATE): `nrpvp_launcher.exe` and the launcher app refuse to start the game when the installed mod is
//! older than the latest published release, because everyone in a match needs the same version. Both go through
//! [`crate::launch`], which calls [`gate`].
//!
//! The installed version comes from `NightreignPvP\VERSION.txt` (its `version X.Y.Z` line, written by
//! `scripts\package-release.ps1`), else from `NightreignPvP\release.json` (written by the launcher app after an install). The latest
//! version is the newest stable release's `release.json` ([`crate::release::latest`]).
//!
//! Decisions (DECISIONS 2026-11-17, RELEASE.md §5c):
//! - installed older than latest, or no version recorded at all (a zip from before the gate) → **blocked**;
//! - installed same or newer → start;
//! - the latest version cannot be learnt (offline, GitHub's limit, an error) → **blocked** too (Leonardo: the game does not open
//!   without the check); the message says to connect and try again, and `launcher.log` says why;
//! - a locally built zip (`version local`, `package-release.ps1` without `-Version`) → start, no network asked (friend-test zips);
//! - `NRPVP_SKIP_VERSION_CHECK=1` → start (developer escape). The developer paths (`scripts\launch.ps1`, `nro`) never come here.

use std::path::Path;

use semver::Version;

/// The build line file inside the mod folder; its `version` line names the release.
pub const VERSION_FILE: &str = "VERSION.txt";
/// The installed release's manifest inside the mod folder, written by the launcher app after an install.
pub const INSTALLED_MANIFEST: &str = "release.json";
/// The `version` value of a zip built on a developer PC (not a published release).
pub const LOCAL_BUILD: &str = "local";
/// Developer escape: set to anything but empty or `0` to skip the gate.
pub const SKIP_ENV: &str = "NRPVP_SKIP_VERSION_CHECK";

/// True when `remote` is a newer version than `local` (both `X.Y.Z`); false when either does not parse, so a damaged value never
/// triggers an update loop or a downgrade.
pub fn newer(remote: &str, local: &str) -> bool {
    match (Version::parse(remote), Version::parse(local)) {
        (Ok(r), Ok(l)) => r > l,
        _ => false,
    }
}

/// What the installed mod says about its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Installed {
    /// A published release.
    Release(Version),
    /// A zip built on a developer PC (`version local`).
    Local,
    /// No version recorded (a zip from before the gate, or the files are damaged).
    Unknown,
}

/// The `version` line of `VERSION.txt` (`version 0.3.4` or `version local`). Kept pure for the unit test.
pub fn parse_version_file(text: &str) -> Installed {
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if !words
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case("version"))
        {
            continue;
        }
        let Some(value) = words.next() else { continue };
        let value = value.trim_end_matches(',');
        if value.eq_ignore_ascii_case(LOCAL_BUILD) {
            return Installed::Local;
        }
        if let Ok(v) = Version::parse(value.trim_start_matches('v')) {
            return Installed::Release(v);
        }
    }
    Installed::Unknown
}

/// The `version` field of the installed `release.json`. Kept pure for the unit test.
pub fn parse_installed_manifest(text: &str) -> Installed {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| Version::parse(v.get("version")?.as_str()?).ok())
        .map_or(Installed::Unknown, Installed::Release)
}

/// The installed mod's version in the game folder `game`: `VERSION.txt` first (it ships with the files, so a zip dropped over an
/// app install wins), then the app's `release.json`.
pub fn installed(game: &Path) -> Installed {
    let mod_dir = game.join(crate::MOD_DIR);
    let from_file = std::fs::read_to_string(mod_dir.join(VERSION_FILE))
        .map(|t| parse_version_file(&t))
        .unwrap_or(Installed::Unknown);
    if from_file != Installed::Unknown {
        return from_file;
    }
    std::fs::read_to_string(mod_dir.join(INSTALLED_MANIFEST))
        .map(|t| parse_installed_manifest(&t))
        .unwrap_or(Installed::Unknown)
}

/// The gate's verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// The installed release is the latest (or newer): start.
    Latest { installed: Version, latest: Version },
    /// Older than the latest, or no version recorded (`installed: None`): do not start.
    Outdated {
        installed: Option<Version>,
        latest: Version,
    },
    /// A zip built on a developer PC: start without asking.
    LocalBuild,
    /// The latest version could not be learnt: do not start (fail closed, Leonardo 2026-11-17).
    Unknown { why: String },
    /// [`SKIP_ENV`] is set: start.
    Skipped,
}

impl Gate {
    pub fn blocks(&self) -> bool {
        matches!(self, Gate::Outdated { .. } | Gate::Unknown { .. })
    }

    /// One line for `launcher.log`.
    pub fn log_line(&self) -> String {
        match self {
            Gate::Latest { installed, latest } => {
                format!("version gate: OK, installed {installed}, latest {latest}")
            }
            Gate::Outdated { installed, latest } => format!(
                "version gate: BLOCKED, installed {}, latest {latest}",
                installed
                    .as_ref()
                    .map_or("unknown".to_string(), |v| v.to_string())
            ),
            Gate::LocalBuild => "version gate: skipped, local build".to_string(),
            Gate::Unknown { why } => {
                format!("version gate: BLOCKED, latest unknown ({why})")
            }
            Gate::Skipped => format!("version gate: skipped ({SKIP_ENV})"),
        }
    }
}

/// The verdict from what is installed and what GitHub said (`Err` = could not ask). Kept pure for the unit test.
pub fn decide(installed: &Installed, latest: Result<&str, &str>) -> Gate {
    if *installed == Installed::Local {
        return Gate::LocalBuild;
    }
    let latest = match latest {
        Err(why) => return Gate::Unknown { why: why.into() },
        Ok(text) => match Version::parse(text) {
            Ok(v) => v,
            Err(_) => {
                return Gate::Unknown {
                    why: format!("the latest release has a damaged version ({text})"),
                }
            }
        },
    };
    match installed {
        Installed::Release(v) if *v >= latest => Gate::Latest {
            installed: v.clone(),
            latest,
        },
        Installed::Release(v) => Gate::Outdated {
            installed: Some(v.clone()),
            latest,
        },
        _ => Gate::Outdated {
            installed: None,
            latest,
        },
    }
}

/// What the player reads when the gate blocks, in words they can act on. `page` = where the new zip is.
pub fn blocked_message(installed: Option<&Version>, latest: &Version, page: &str) -> String {
    let what = match installed {
        Some(v) => format!("Your Nightclash version is v{v}, but v{latest} is out."),
        None => format!("Your Nightclash is an old version (v{latest} is out)."),
    };
    format!(
        "{what} Everyone in a match needs the same version, so the game will not start with this one.\n\n\
         To update, do one of these:\n\
         - Open the Nightclash launcher app (NightreignPvP.exe): it updates the mod by itself, then press Play.\n\
         - Or download the new NightreignPvP.zip from {page} and drag its files into the game folder (choose \"Replace the files\")."
    )
}

/// What the player reads when the latest version could not be learnt (`why` = what went wrong, in player words).
pub fn unknown_message(why: &str) -> String {
    format!(
        "Nightclash could not check for the latest version, so the game will not start.\n\n{why}\n\n\
         The game only starts after this check (everyone in a match needs the same version). Connect to the internet and try again."
    )
}

fn skipped_by_env() -> bool {
    std::env::var_os(SKIP_ENV).is_some_and(|v| !v.is_empty() && v != "0")
}

/// The gate for the game folder `game`. Asks GitHub (or `NRPVP_RELEASE_DIR`) for the latest release unless the install is a local
/// build or [`SKIP_ENV`] is set.
pub fn gate(game: &Path) -> Gate {
    if skipped_by_env() {
        return Gate::Skipped;
    }
    let installed = installed(game);
    if installed == Installed::Local {
        return Gate::LocalBuild;
    }
    let latest = crate::release::latest();
    decide(
        &installed,
        latest
            .as_ref()
            .map(|m| m.version.as_str())
            .map_err(String::as_str),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions_never_downgrade_or_loop() {
        assert!(newer("0.10.0", "0.2.0"));
        assert!(!newer("0.2.0", "0.10.0"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("broken", "0.2.0"));
        assert!(!newer("0.3.1", "broken"));
    }

    #[test]
    fn version_file_line_is_read_and_old_files_are_unknown() {
        assert_eq!(
            parse_version_file(
                "version 0.3.4\r\nbuilt 2026-11-17 10:00 UTC, commit abc (clean)\r\n"
            ),
            Installed::Release(v("0.3.4"))
        );
        assert_eq!(
            parse_version_file("Version v0.10.2"),
            Installed::Release(v("0.10.2"))
        );
        assert_eq!(
            parse_version_file("version local\r\nbuilt ..."),
            Installed::Local
        );
        // A zip from before the gate: one build line, no version.
        assert_eq!(
            parse_version_file("built 2026-11-13 15:39 UTC, commit b365217 (dirty), nrpvp.dll 6C24A4FD801F9604, me3 0.11.0\r\n"),
            Installed::Unknown
        );
        assert_eq!(parse_version_file("version"), Installed::Unknown);
        assert_eq!(parse_version_file("version 1.2"), Installed::Unknown);
        assert_eq!(parse_version_file(""), Installed::Unknown);
    }

    #[test]
    fn installed_manifest_gives_its_version() {
        assert_eq!(
            parse_installed_manifest(r#"{"schema":1,"version":"0.3.2","tag":"v0.3.2"}"#),
            Installed::Release(v("0.3.2"))
        );
        assert_eq!(parse_installed_manifest("{}"), Installed::Unknown);
        assert_eq!(parse_installed_manifest("not json"), Installed::Unknown);
    }

    #[test]
    fn decide_latest_outdated_unknown() {
        let rel = |s: &str| Installed::Release(v(s));
        // Latest (same or newer than published).
        assert!(!decide(&rel("0.3.4"), Ok("0.3.4")).blocks());
        assert_eq!(
            decide(&rel("0.3.5"), Ok("0.3.4")),
            Gate::Latest {
                installed: v("0.3.5"),
                latest: v("0.3.4")
            }
        );
        // Outdated: semver order, not text order.
        assert_eq!(
            decide(&rel("0.3.9"), Ok("0.3.10")),
            Gate::Outdated {
                installed: Some(v("0.3.9")),
                latest: v("0.3.10")
            }
        );
        // No version recorded = older than the gate = blocked.
        assert_eq!(
            decide(&Installed::Unknown, Ok("0.3.4")),
            Gate::Outdated {
                installed: None,
                latest: v("0.3.4")
            }
        );
        // The latest version is not known: blocked (fail closed), whatever is installed, even the newest.
        for inst in [rel("0.1.0"), rel("99.0.0"), Installed::Unknown] {
            let g = decide(&inst, Err("Could not reach GitHub."));
            assert!(g.blocks(), "{g:?}");
            assert!(
                g.log_line().contains("BLOCKED")
                    && g.log_line().contains("Could not reach GitHub.")
            );
        }
        assert!(decide(&rel("0.1.0"), Ok("garbage")).blocks());
        // A local build never blocks and needs no answer.
        assert_eq!(decide(&Installed::Local, Ok("9.9.9")), Gate::LocalBuild);
        assert_eq!(decide(&Installed::Local, Err("offline")), Gate::LocalBuild);
    }

    #[test]
    fn blocked_message_says_what_to_do() {
        let m = blocked_message(Some(&v("0.3.1")), &v("0.3.4"), "https://x/releases/latest");
        assert!(m.contains("v0.3.1") && m.contains("v0.3.4"));
        assert!(m.contains("NightreignPvP.exe") && m.contains("https://x/releases/latest"));
        assert!(blocked_message(None, &v("0.3.4"), "p").contains("old version"));
        let u = unknown_message("Could not reach GitHub.");
        assert!(
            u.contains("will not start")
                && u.contains("Could not reach GitHub.")
                && u.contains("Connect")
        );
    }

    #[test]
    fn installed_reads_the_version_file_before_the_app_manifest() {
        let dir = std::env::temp_dir().join(format!("nrpvp-gate-test-{}", std::process::id()));
        let m = dir.join(crate::MOD_DIR);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&m).unwrap();
        assert_eq!(installed(&dir), Installed::Unknown);
        std::fs::write(m.join(INSTALLED_MANIFEST), r#"{"version":"0.3.2"}"#).unwrap();
        assert_eq!(installed(&dir), Installed::Release(v("0.3.2")));
        std::fs::write(m.join(VERSION_FILE), "built ..., no version\r\n").unwrap();
        assert_eq!(installed(&dir), Installed::Release(v("0.3.2")));
        std::fs::write(m.join(VERSION_FILE), "version 0.3.4\r\nbuilt ...\r\n").unwrap();
        assert_eq!(installed(&dir), Installed::Release(v("0.3.4")));
        std::fs::write(m.join(VERSION_FILE), "version local\r\n").unwrap();
        assert_eq!(installed(&dir), Installed::Local);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
