# 2026-11-26 Arena Trio vs bots: the screen stayed black 30 s before round 2 (ARENA-SPAWN)

### 2026-11-26 — Arena Trio vs bots (Leonardo, player install v0.4.3 = commit 5c23e7e, nrpvp.dll 5B44B83844E0E844). Log: `<Game>\NightreignPvP\logs\nrpvp_2026-11-26_17-32-12.log`: **FAIL** (match won 2-0, but blind for 30 s)

Game 1.03.3, me3 0.11.0, player build. Bots On, Arena card, Trio, Commence at 17:35:01 UTC.

- **Symptom (Leonardo):** the screen was black while the round had already started.
- **Timeline:** `Round 1: you won (1-0)` at 17:36:55.2; reset at 17:37:00.26 (LOCAL and both allies put back on their spawns, the
  three new enemies on theirs by 17:37:01.6); `Round 2: fight!` only at 17:37:31.23 = 36 s after the round end, exactly `FADE_IN_MS`
  (6 s) + `RIVAL_WAIT_MS` (30 s). Meanwhile `T-033 hit: a rival's hit on LOCAL` at :12, :17, :19, :28: the bots are not frozen.
- **Cause:** ally slot 2 (`0x...10E00004`) never logged `stands on its spawn` after the reset. Its reset move was written (`put at (-15,
  3, -4) from (-1, -2, -120)`) but did not hold: at the round 2 end it was parked `from (-13, -2, -120)`, still at its park. So
  `arena_all_ready` stayed false and the fade-in waited the full 30 s. The other ally, moved in the same tick, held. Why the game undid
  the write is UNKNOWN (TRAPS "A written move can be undone by the game").
- **Fix (branch `arena-spawn-retry`, PR to `dev`):** behind the black screen (before round 1, and from a reset to the fade-in) each bot
  off its spawn by more than 4 m is put back, at most once a second, and LOCAL off its spawn by more than 3 m is moved again; after a
  reset a missing bot holds the screen at most `RESET_RIVAL_WAIT_MS` (5 s) past the fade-in time. Unit tests: 722 pass.
- **Still to run in game:** one Arena Trio (or Duo) vs bots match to the end; PASS = every `Round N: fight!` within ~7 s of the round
  end and no `T-033 hit` between a round end and the next `fight!`. A `put back` line is fine (the retry at work).

Earlier dev log (`dist\logs\nrpvp_2026-11-25_23-38-13.log`, 23:59): a match ended ~7 s after round 1 while the screen was black between
rounds; no 30 s wait in that log, not counted as this bug.
