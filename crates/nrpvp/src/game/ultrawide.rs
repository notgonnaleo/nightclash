//! UW-1 (both builds, on by default; `ultrawide = false` turns it off): no black bars on a screen wider (or narrower) than 16:9. Game 1.03.3 / exe 1.3.3.0;
//! the reading is in MEMORY_MAP §30 ("Render size and the 16:9 letterbox").
//!
//! When the display settings are applied, the game turns the window size into the render size (`+0x110` width, `+0x114` height of
//! the display object) by fitting it to an aspect pair from a settings record (`+0x4B0` : `+0x4B4`, 16 : 9). When either number is 0
//! it keeps the full window size. The `je` at `exe+0x1B8B286` (`74 66`, "denominator is 0 → full size") becomes `jmp` (`EB 66`), so the
//! render size is always the window size. Same one-byte change as the community fix "nightreign-ultrawidefix" (a fork of techiew's
//! MIT UltrawideFix); our code is our own.
//!
//! The patch is written once at startup, before the game creates its window, so the first apply already sees it. A change of the
//! window mode or resolution in the game's menu applies it again. On a 16:9 window nothing changes. Log lines start with `ultrawide:`.

use std::sync::atomic::{AtomicU8, Ordering};

/// The `je rel8` whose taken branch keeps the full window size. `[VERIFIED: static disassembly of exe 1.3.3.0, 2026-11-25, MEMORY_MAP §30]`
const SITE: usize = 0x1B8_B286;
const VANILLA: [u8; 2] = [0x74, 0x66];
const PATCHED: [u8; 2] = [0xEB, 0x66];

/// 0 = not tried, 1 = patched by us, 2 = found already patched (another mod), 3 = unexpected bytes (left alone), 4 = write failed.
static STATE: AtomicU8 = AtomicU8::new(0);

/// What the two bytes at the site say. Pure, tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Site {
    Vanilla,
    Patched,
    Unknown,
}

pub(crate) fn classify(bytes: [u8; 2]) -> Site {
    match bytes {
        VANILLA => Site::Vanilla,
        PATCHED => Site::Patched,
        _ => Site::Unknown,
    }
}

/// Write `byte` as the site's opcode (the protection is widened for the store and put back).
fn write_opcode(addr: usize, byte: u8) -> Result<(), String> {
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut old = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: `addr` is the opcode byte of a `je rel8` in the game's .text (checked by the caller); a one-byte store is atomic, so a
    // thread running the code sees the old or the new opcode, and both decode with the same rel8 operand.
    unsafe {
        VirtualProtect(
            addr as *const std::ffi::c_void,
            1,
            PAGE_EXECUTE_READWRITE,
            &mut old,
        )
        .map_err(|e| format!("VirtualProtect: {e}"))?;
        std::ptr::write_volatile(addr as *mut u8, byte);
        let mut tmp = PAGE_PROTECTION_FLAGS(0);
        let _ = VirtualProtect(addr as *const std::ffi::c_void, 1, old, &mut tmp);
        let _ = FlushInstructionCache(
            GetCurrentProcess(),
            Some(addr as *const std::ffi::c_void),
            1,
        );
    }
    Ok(())
}

/// Turn the patch on or off. Returns one line for the log or the console.
pub(crate) fn set(base: usize, on: bool) -> String {
    let addr = base + SITE;
    let Some(bytes) = super::mem::read::<[u8; 2]>(addr) else {
        STATE.store(4, Ordering::Relaxed);
        return format!("ultrawide: exe+0x{SITE:X} is not readable");
    };
    let (want, from) = if on {
        (PATCHED, Site::Vanilla)
    } else {
        (VANILLA, Site::Patched)
    };
    match classify(bytes) {
        s if s == from => match write_opcode(addr, want[0]) {
            Ok(()) => {
                STATE.store(if on { 1 } else { 0 }, Ordering::Relaxed);
                format!(
                    "ultrawide: {} (exe+0x{SITE:X} {:02X} -> {:02X}); takes effect the next time the display settings are applied",
                    if on { "on" } else { "off" },
                    bytes[0],
                    want[0]
                )
            }
            Err(e) => {
                STATE.store(4, Ordering::Relaxed);
                format!("ultrawide: write failed: {e}")
            }
        },
        Site::Unknown => {
            STATE.store(3, Ordering::Relaxed);
            format!(
                "ultrawide: unexpected bytes {bytes:02X?} at exe+0x{SITE:X} (another game version?); left alone"
            )
        }
        _ => {
            if on {
                STATE.store(2, Ordering::Relaxed);
            }
            format!(
                "ultrawide: already {} (bytes {bytes:02X?})",
                if on { "on (another mod?)" } else { "off" }
            )
        }
    }
}

/// Startup (both builds): patch once when `ultrawide = true`.
pub(crate) fn install(base: usize) {
    let line = set(base, true);
    if STATE.load(Ordering::Relaxed) == 1 || STATE.load(Ordering::Relaxed) == 2 {
        tracing::info!("{line}");
    } else {
        tracing::warn!("{line}");
    }
}

/// Console `ultrawide`: the site's bytes now.
pub(crate) fn describe(base: usize) -> String {
    match super::mem::read::<[u8; 2]>(base + SITE) {
        Some(b) => format!(
            "ultrawide: {} (exe+0x{SITE:X} = {b:02X?}, startup state {})",
            match classify(b) {
                Site::Vanilla => "off",
                Site::Patched => "on",
                Site::Unknown => "unknown bytes",
            },
            STATE.load(Ordering::Relaxed)
        ),
        None => format!("ultrawide: exe+0x{SITE:X} is not readable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_site() {
        assert_eq!(classify([0x74, 0x66]), Site::Vanilla);
        assert_eq!(classify([0xEB, 0x66]), Site::Patched);
        assert_eq!(classify([0x74, 0x67]), Site::Unknown);
        assert_eq!(classify([0x90, 0x90]), Site::Unknown);
    }

    #[test]
    fn toggles_a_copy_of_the_site() {
        // A heap copy of the bytes around the site: the same code path the game image takes.
        let mut buf = vec![0x45u8, 0x85, 0xC0, 0x74, 0x66, 0x8B, 0xD0];
        let base = buf.as_mut_ptr() as usize + 3 - SITE;
        let r = set(base, true);
        assert!(r.contains("on"), "{r}");
        assert_eq!(buf[3..5], PATCHED);
        let r = set(base, true);
        assert!(r.contains("already on"), "{r}");
        let r = set(base, false);
        assert!(r.contains("off"), "{r}");
        assert_eq!(buf[3..5], VANILLA);
        buf[3] = 0x90;
        let r = set(base, true);
        assert!(r.contains("unexpected"), "{r}");
        assert_eq!(buf[3], 0x90);
    }
}
