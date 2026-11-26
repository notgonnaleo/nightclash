# 2026-11-25 Ultrawide: no 16:9 black bars (UW-1)

**Build:** branch `ultrawide` (on 423ffdb, dev DLL of 2026-11-25 19:40), game 1.03.3 (exe 1.3.3.0), SC 1.1.3, me3 0.11.0. One game on
one PC through `nro`, `nro desk` in a Terminal-panel tab. Monitor 2752x1152 logical (21:9 class), the game in BORDERLESS mode (its own
`GraphicsConfig.xml`, not changed for the test).

**Run** `runs\1795648220-session`: `nro launch --world --set mode="offline" --set ultrawide=true`.
- Log: `ultrawide: on (exe+0x1B8B286 74 -> EB)` at startup; console `ultrawide`: `[EB, 66]`, startup state 1.
- Screenshot `uw_on_roundtable.png` (1720x720, 2.39:1): **the Roundtable fills the whole width, no bars**, with no change of the
  window mode or resolution after the launch (the community fix needs one; ours patches before the window exists).
- HUD: the rune counter sits bottom right inside the centred 16:9 area (about x 1470 of 1720), not stretched.
- Leonardo looked at the game: "it worked".

**Verdict: PASS** for UW-1 acceptance 1 (whole window, log line) and 3 (no resolution toggle). Not seen: an expedition, the console
`ultrawide 0` path back to bars, cutscenes and loading screens.
