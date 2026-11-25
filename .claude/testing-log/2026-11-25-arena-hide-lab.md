# 2026-11-25 Arena: can a living player be hidden for the team spectate? (ARENA-6)

**Build:** branch `arena-duo-trio` (dev DLL of 2026-11-24 23:39, `dist\gamedata` rebuilt 2026-11-24 with events 99100921 / 99100922),
game 1.03.3 (exe 1.3.3.0), SC 1.1.3, me3 0.11.0. Two emulated instances on one PC, `nro desk` in a Terminal-panel tab, Steam closed.

**Run** `runs95574581-mi-arena-hide-lab-i1` / `...585-...-i2`, scenario `scenarios/mi-arena-hide-lab.toml`: **steps PASS** (a lab: the
verdict is the screenshots). The Arena 1v1 started with the queue as in `mi-arena-queue`; once round 1 was armed instance 1 (team A,
at (-15, 0)) turned its flags on and moved itself:

| Stage | Instance 1 (own screen) | Instance 2 (sees player 1) | Instance 2 `players_pos` for player 1 |
|---|---|---|---|
| before | own character drawn | drawn at its spawn, 30 m off | (-15.00, 1.87, 0.00) |
| ghost 10007521 ON (no collision, no gravity) | | still drawn (`p2_ghost`) | |
| hide 10007522 ON (Change Character Enable State Disabled) | **own character gone, camera fine, sees player 2** (`p1_hidden`) | **still drawn** (`p2_hidden`) | (-15.00, 1.87, 0.00) |
| `tp 0 3 8` while hidden (17 m) | **the view moved with the body** (`p1_hidden_moved`), `pos` (0.00, 3.00, 8.00): no fall | **still drawn at the old place** (`p2_hidden_moved`) | **(-15.00, 1.87, 0.00): no update** |
| both OFF | character back (`p1_back`), `pos` y 2.65 (on the floor) | | (-0.00, 2.65, 8.00): caught up |

**Verdict:** the disable hides the body only on its own game, but it also **stops the body's position updates**, so the others keep
drawing it where it was disabled. The camera follows a disabled body. Hence the built order (code changed after this run): beaten ->
frozen, ghost, moved to the far bench (120 m out, behind its own spawn); 1.5 s later (the position has reached the others) hidden;
0.7 s later its body follows a teammate every tick. The others see a still figure on the far bench; the beaten player watches the
team. Not yet seen: that whole flow in a 2v2 (four instances), and the bench figure from the fight's distance.
