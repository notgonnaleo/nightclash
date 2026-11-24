//! nrpvp_launcher.exe: the one file a player double-clicks in the game folder. The work is in the library (lib.rs).
#![windows_subsystem = "windows"]

const TITLE: &str = "Nightclash";

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        hwnd: *mut core::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        kind: u32,
    ) -> i32;
}

const MB_ICONERROR: u32 = 0x10;
const MB_YESNO: u32 = 0x04;
const IDYES: i32 = 6;

/// A message box; answers the button pressed.
fn message(text: &str, kind: u32) -> i32 {
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let (t, c) = (wide(text), wide(TITLE));
    // SAFETY: both buffers are valid NUL-terminated UTF-16 strings that outlive the call.
    unsafe { MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), kind) }
}

/// Opens the download page: a web page in the default browser, a folder (NRPVP_RELEASE_DIR tests) in Explorer.
fn open_page(page: &str) {
    let mut cmd = if page.starts_with("https://") {
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        std::process::Command::new("explorer")
    };
    let _ = cmd.arg(page).spawn();
}

fn main() {
    let game = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));
    let result = match game {
        Some(game) => nrpvp_launch::launch(&game),
        None => Err("Could not find my own folder.".to_string().into()),
    };
    match result {
        Ok(()) => {}
        // LAUNCH-GATE: an old install; offer the download page.
        Err(nrpvp_launch::LaunchError::Outdated {
            message: text,
            page,
        }) => {
            let ask = format!("{text}\n\nOpen the download page now?");
            if message(&ask, MB_ICONERROR | MB_YESNO) == IDYES {
                open_page(&page);
            }
        }
        Err(why) => {
            message(&why.to_string(), MB_ICONERROR);
        }
    }
}
