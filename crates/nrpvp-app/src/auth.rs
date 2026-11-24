//! Launchers up to v0.1.x could save a GitHub token for private releases. Releases are public now
//! (`notgonnaleo/nightclash`, formerly `notgonnaleo/nightreign-pvp`) and no token is ever sent; a leftover saved token only broke updates (GitHub answers 401
//! for an expired token even on a public repository), so startup deletes the file.
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    let root = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(root).join("NightreignPvP/github-token.dpapi"))
}

/// Removes a token saved by an older launcher, if any. Errors are ignored: the file is never read.
pub fn forget_saved_token() {
    if let Some(path) = path() {
        let _ = std::fs::remove_file(path);
    }
}
