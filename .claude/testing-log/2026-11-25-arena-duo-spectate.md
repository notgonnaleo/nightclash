# 2026-11-25 Arena 2v2 with the team spectate, four instances, played by hand (ARENA-6)

**Build:** branch `arena-duo-trio`, dev DLL of 2026-11-24 23:51 (far bench at 120 m, hide 1.5 s after the bench move, follow 0.7 s
later), `dist\gamedata` of 2026-11-24 (events 99100921 / 99100922). Game 1.03.3 (exe 1.3.3.0), SC 1.1.3, me3 0.11.0. Four emulated
instances (`scripts\four-instances.ps1 -Prepare`, 23 GB free), runs `1795575672..694-mi-arena-duo-i1..i4`. Unit tests 711 PASS,
clippy dev and player clean.

**`nro run scenarios\mi-arena-duo.toml` ABORTED** at the match wait: instance 4 was told to queue while still loading
(`REFUSED: load into the Roundtable first`), so the queue stayed at 3/4. The games stayed up (`keep`); `mm_queue_arena_duo` on
instance 4 filled it (ALL PRESENT 4/4), and **Leonardo played the match by hand** (real hits; `hp_set 1` only for instance 3 in round
1). Also fixed first: a form feed in the scenario's line-7 comment (`scripts\f` written as an escape), which TOML refused.

**Result: PASS for the team spectate** (logs of all four, `players_pos`, screenshots `p3_watching`, `p1_sees_p3_bench`,
`p4_with_p3_following`, `p2_round2`):
- Teams 1+2 (A) vs 3+4 (B); every game `sides: LOCAL + 1 teammate(s) vs 2 rival(s)`. Session ids: i1 0x6001, i2 0x6004, i3 0x6003,
  i4 0x6002.
- Five beatings with a teammate still up, each the same chain: `frozen, watching the team` -> `-> bench (±120, 3, z)` -> `on the
  bench; hidden in 1500 ms` -> `spectator body hidden on` (1.6-1.7 s later) -> `watching teammate <its teammate>` (0.6 s later):
  round 1 i3 (watching i4) and i1 (watching i2); round 2 i4 (watching i3) and i2 (watching i1); round 3 i1 (watching i2).
- **The others see the spectator on the far bench, the spectator is on its teammate:** round 1, i3 `pos` (-19.2, 1.55, -1.0) next to
  its teammate i4 (-19.8), while i1's `players_pos` had i3 at (120, 3, 4); round 2, i4 `pos` = i3's (15, 2.18, 4) while i2's game had
  i4 at (120, 3, 0). i3's screen showed its teammate with the HUD of a living player (`p3_watching`); i4's screen, the watched one,
  showed no second body on it (`p4_with_p3_following`).
- Every reset: `-> spawn`, `spectator ghost ... off`, `spectator body hidden off`, the next round `everyone up`; the others then saw
  the spawn positions again.
- Rounds: 1-0 for A, 1-1, 2-1 for B: `match over, WIN 2-1` on i3 / i4 and `LOSE 1-2` on i1 / i2 within 0.04 s; all four back at the
  Roundtable, queue idle, the scoreboard summary shown.
- Moves refused mid-attack (`Physics+0x50 does not read as a heading`) for about a second, then the forced move went through (i2's
  bench move 1.2 s after its beating), as the reset moves always did.

**Seen and changed after the run (unit tests only):** when a team's last player falls, its spectator went to the empty bench for the
fade-out (`no teammate to watch: to the bench`); now it stays on the teammate watched last. A forced bench move that still failed
would have hidden the body where it stood (the others would see it there); the hide now waits for a written bench move.

**Bug found in the reset logs (older than ARENA-6, fixed after the run, unit test only):** twice a round's winner was frozen
mid-attack at the round end; its move back to the spawn was refused (`Physics+0x50 does not read as a heading`, a tilted rotation)
until it was forced after 1 s, and the screen comes back exactly 1 s after the reset, so the forced move lost the race: i2 jumped
7.4 m at 03:07:28.02, 0.2 s after "Round 2: fight!", and i3 jumped 33.8 m at 03:08:25.05, 0.3 s after "Round 3: fight!" (the
next round was live with them where the last one ended). Every other reset move landed at once. Now the fade-in waits until LOCAL's
move is written (at most 3 s, `PLACE_WAIT_MS`).

Not covered: the switch inputs (mouse buttons / D-pad) to change the watched teammate, Trio, a bot match, real Steam.
