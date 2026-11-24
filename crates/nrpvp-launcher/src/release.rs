//! Where releases come from, and the `release.json` manifest that describes one. Shared by the launcher app (install, updates)
//! and by [`crate::launch`]'s version gate ([`crate::version`]), so both ask GitHub the same way.
//!
//! Player releases are GitHub releases of the **public** source repository ([`RELEASES_REPO`], `notgonnaleo/nightclash`,
//! named `notgonnaleo/nightreign-pvp` from 2026-11-23 to 2026-11-24, GitHub redirects the old name; before that `aspherya/nightreign-pvp-releases`). Every release carries three assets: `release.json` (this
//! manifest), the player zip, and the launcher exe. GitHub release/asset API requests download the manifest and assets without
//! a token (never send one: an expired token gets 401 even on a public repository); their SHA-256 is checked before install.
//! `scripts\publish-ci-release.ps1` uploads the release from CI.
//!
//! For testing without GitHub: `NRPVP_RELEASE_DIR=<folder holding release.json and the zip>` reads a local folder instead;
//! `NRPVP_RELEASES_REPO=<owner/name>` points at another repository.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const RELEASES_REPO: &str = match option_env!("NRPVP_RELEASES_REPO") {
    Some(repo) => repo,
    None => "notgonnaleo/nightclash",
};
/// Seamless Co-op for Nightreign, by Yui. The GitHub repository holds only data files today (no releases); the mod itself is
/// published on Nexus Mods. The launcher installs from a GitHub release if one ever appears, else guides the Nexus download.
pub const SC_GITHUB_REPO: &str = "yuiamoroll/NightreignSeamlessCoopRelease";
pub const SC_NEXUS_URL: &str = "https://www.nexusmods.com/eldenringnightreign/mods/3?tab=files";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamePin {
    /// e.g. "1.03.3"
    pub version: String,
    pub exe_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScPin {
    /// e.g. "1.1.3"
    pub version: String,
    pub nrsc_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    /// e.g. "0.2.0"
    pub version: String,
    /// The GitHub tag, e.g. "v0.2.0".
    pub tag: String,
    /// "YYYY-MM-DD"
    pub published: String,
    /// Patch notes, plain text; blank lines separate paragraphs, lines starting with "- " are list items.
    #[serde(default)]
    pub notes: String,
    pub zip: Asset,
    #[serde(default)]
    pub launcher: Option<Asset>,
    pub game: GamePin,
    pub seamless_coop: ScPin,
    /// Players per match in this build (mm_max_players).
    #[serde(default)]
    pub match_players: Option<u32>,
}

pub enum Source {
    GitHub(String),
    Local(PathBuf),
}

pub fn source() -> Source {
    if let Some(dir) = std::env::var_os("NRPVP_RELEASE_DIR") {
        return Source::Local(PathBuf::from(dir));
    }
    Source::GitHub(
        std::env::var("NRPVP_RELEASES_REPO").unwrap_or_else(|_| RELEASES_REPO.to_string()),
    )
}

pub fn releases_page() -> String {
    match source() {
        Source::GitHub(repo) => format!("https://github.com/{repo}/releases"),
        Source::Local(dir) => dir.display().to_string(),
    }
}

/// How long a small request (release info, `release.json`) may take in all. The version gate waits on it before the game starts.
const SMALL_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// `total` = the whole request's time limit; `None` for large downloads (the zip), which may take minutes.
/// Where a player downloads the newest zip by hand (the version gate's message).
pub fn download_page() -> String {
    match source() {
        Source::GitHub(repo) => format!("https://github.com/{repo}/releases/latest"),
        Source::Local(dir) => dir.display().to_string(),
    }
}

fn client(total: Option<Duration>) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!(
            "NightreignPvP-Launcher/",
            env!("CARGO_PKG_VERSION")
        ))
        .connect_timeout(Duration::from_secs(8))
        .timeout(total)
        .build()
        .map_err(|e| format!("Could not start the download client: {e}"))
}

#[derive(Deserialize)]
struct GitHubAsset {
    id: u64,
    name: String,
}
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GitHubAsset>,
}

fn github_url(repo: &str, endpoint: &[&str]) -> Result<reqwest::Url, String> {
    let parts: Vec<_> = repo.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        })
    {
        return Err("The GitHub repository must be owner/name.".into());
    }
    let mut url = reqwest::Url::parse("https://api.github.com").map_err(|e| e.to_string())?;
    url.path_segments_mut()
        .map_err(|_| "Invalid GitHub URL.")?
        .extend(["repos", parts[0], parts[1]])
        .extend(endpoint);
    Ok(url)
}

fn github_get(
    repo: &str,
    endpoint: &[&str],
    binary: bool,
    total: Option<Duration>,
) -> Result<reqwest::blocking::Response, String> {
    let response = client(total)?
        .get(github_url(repo, endpoint)?)
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header(
            "Accept",
            if binary {
                "application/octet-stream"
            } else {
                "application/vnd.github+json"
            },
        )
        .send()
        .map_err(|_| "Could not reach GitHub. Check your connection and try again.")?;
    match response.status().as_u16() {
        200..=299 => Ok(response),
        403 | 429 => {
            Err("GitHub's download limit was reached for now. Try again in an hour.".into())
        }
        404 => Err("No published release was found yet. Try again later.".into()),
        status => Err(format!("GitHub answered {status}; try again later.")),
    }
}

fn github_release(repo: &str, tag: Option<&str>) -> Result<GitHubRelease, String> {
    let endpoint = match tag {
        Some(tag) => vec!["releases", "tags", tag],
        None => vec!["releases", "latest"],
    };
    let release: GitHubRelease = github_get(repo, &endpoint, false, Some(SMALL_REQUEST_TIMEOUT))?
        .json()
        .map_err(|_| "GitHub returned invalid release information.")?;
    if release.draft || release.prerelease {
        return Err("This build has not been published as a stable release.".into());
    }
    Ok(release)
}

fn github_asset(
    repo: &str,
    release: &GitHubRelease,
    name: &str,
    total: Option<Duration>,
) -> Result<reqwest::blocking::Response, String> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or_else(|| format!("The release is missing {name}."))?;
    // Construct the API URL from the numeric asset ID; never follow URLs supplied by a manifest.
    github_get(
        repo,
        &["releases", "assets", &asset.id.to_string()],
        true,
        total,
    )
}

/// The latest release's manifest.
pub fn latest() -> Result<Manifest, String> {
    let text = match source() {
        Source::Local(dir) => std::fs::read_to_string(dir.join("release.json"))
            .map_err(|e| format!("No release.json in {}: {e}", dir.display()))?,
        Source::GitHub(repo) => {
            let release = github_release(&repo, None)?;
            let text = github_asset(&repo, &release, "release.json", Some(SMALL_REQUEST_TIMEOUT))?
                .text()
                .map_err(|_| "Could not read the release manifest.")?;
            if parse(&text)?.tag != release.tag_name {
                return Err("The manifest tag does not match the GitHub release.".into());
            }
            text
        }
    };
    parse(&text)
}

pub fn parse(text: &str) -> Result<Manifest, String> {
    let m: Manifest =
        serde_json::from_str(text).map_err(|e| format!("The release info is damaged: {e}"))?;
    if m.schema != 1 {
        return Err(format!(
            "This release needs a newer launcher (release format {}). Download the new launcher from the releases page.",
            m.schema
        ));
    }
    Ok(m)
}

/// Downloads (or, for a local source, copies) `asset` of release `m` into `dest`, calling `progress(done, total)`, and checks its
/// SHA-256 against the manifest.
pub fn fetch_asset(
    m: &Manifest,
    asset: &Asset,
    dest: &Path,
    progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let reader: Box<dyn Read> = match source() {
        Source::Local(dir) => Box::new(
            std::fs::File::open(dir.join(&asset.name))
                .map_err(|e| format!("Could not open {}: {e}", asset.name))?,
        ),
        Source::GitHub(repo) => {
            let release = github_release(&repo, Some(&m.tag))?;
            if release.tag_name != m.tag {
                return Err("The GitHub release tag changed.".into());
            }
            Box::new(github_asset(&repo, &release, &asset.name, None)?)
        }
    };
    let hash = copy_hashed(reader, dest, asset.size, progress)?;
    if !hash.eq_ignore_ascii_case(&asset.sha256) {
        let _ = std::fs::remove_file(dest);
        return Err(format!(
            "The download of {} is damaged (checksum differs). Try again.",
            asset.name
        ));
    }
    Ok(())
}

/// GET `url` (redirects followed); an error unless 2xx.
pub fn open_url(url: &str) -> Result<reqwest::blocking::Response, String> {
    let resp = client(None)?
        .get(url)
        .send()
        .map_err(|e| format!("Download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Download failed: the server answered {} for {url}",
            resp.status()
        ));
    }
    Ok(resp)
}

/// Streams `reader` into `dest`, returns the SHA-256 (upper-case hex).
pub fn copy_hashed(
    mut reader: impl Read,
    dest: &Path,
    total: u64,
    mut progress: impl FnMut(u64, u64),
) -> Result<String, String> {
    let mut out = std::fs::File::create(dest)
        .map_err(|e| format!("Could not write {}: {e}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Download interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])
            .map_err(|e| format!("Could not write {}: {e}", dest.display()))?;
        done += n as u64;
        progress(done, total.max(done));
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(hex(&hasher.finalize()))
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let f =
        std::fs::File::open(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, f);
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher).map_err(|e| e.to_string())?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// A zip asset of the latest GitHub release of Seamless Co-op for Nightreign, if there is one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScRelease {
    pub tag: String,
    pub name: String,
    pub url: String,
    pub size: u64,
}

pub fn sc_github_release() -> Result<Option<ScRelease>, String> {
    #[derive(Deserialize)]
    struct Rel {
        tag_name: String,
        assets: Vec<RelAsset>,
    }
    #[derive(Deserialize)]
    struct RelAsset {
        name: String,
        browser_download_url: String,
        size: u64,
    }
    let url = format!("https://api.github.com/repos/{SC_GITHUB_REPO}/releases/latest");
    let resp = client(Some(SMALL_REQUEST_TIMEOUT))?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub answered {}", resp.status()));
    }
    let rel: Rel = resp.json().map_err(|e| e.to_string())?;
    Ok(rel
        .assets
        .into_iter()
        .find(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        .map(|a| ScRelease {
            tag: rel.tag_name,
            name: a.name,
            url: a.browser_download_url,
            size: a.size,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_asset_urls_stay_on_github_and_escape_tag_paths() {
        let url = github_url("notgonnaleo/nightclash", &["releases", "tags", "v1/extra"]).unwrap();
        assert_eq!(url.host_str(), Some("api.github.com"));
        assert!(url.path().ends_with("v1%2Fextra"));
        for repo in [
            "https://evil.example/repo",
            "owner/repo/extra",
            "../repo",
            "owner/repo?token=secret",
        ] {
            assert!(github_url(repo, &["releases"]).is_err());
        }
    }

    const SAMPLE: &str = r#"{
      "schema": 1, "version": "0.2.0", "tag": "v0.2.0", "published": "2026-11-06",
      "notes": "First public build.\n\n- Solo Battle Royale",
      "zip": { "name": "NightreignPvP.zip", "sha256": "AB", "size": 10 },
      "game": { "version": "1.03.3", "exe_sha256": "95" },
      "seamless_coop": { "version": "1.1.3", "nrsc_sha256": "24" },
      "match_players": 2
    }"#;

    #[test]
    fn parses_a_manifest_without_optional_fields() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.tag, "v0.2.0");
        assert!(m.launcher.is_none());
        assert_eq!(m.match_players, Some(2));
    }

    #[test]
    fn a_newer_schema_asks_for_a_new_launcher() {
        let err = parse(&SAMPLE.replace("\"schema\": 1", "\"schema\": 2")).unwrap_err();
        assert!(err.contains("newer launcher"));
    }

    #[test]
    fn copy_hashed_matches_sha256() {
        let dest = std::env::temp_dir().join(format!("nrpvp-app-hash-{}", std::process::id()));
        let mut seen = 0;
        let h = copy_hashed(&b"abc"[..], &dest, 3, |d, _| seen = d).unwrap();
        assert_eq!(
            h,
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
        assert_eq!(seen, 3);
        assert_eq!(sha256_file(&dest).unwrap(), h);
        let _ = std::fs::remove_file(dest);
    }
}
