"""Append our event-script events to an ER/NR EMEVD (decompressed .emevd), and replace existing events' bodies, in place. Used by
scripts/build-gamedata.ps1 (UI-2, OFF-4, M5-3).

Usage: python emevd_add_events.py <file.emevd> <events.json>

events.json: {"events": [{"id": 99100300, "flag": 10007100, "message": 99100200, "display": "dialog" | "blinking" | "status"}, ...]}
Each event: Set Network Sync State Disabled [2000:2] (FS7-1: per game) -> IF Event Flag(MAIN, ON, flag) [3:0] -> display(message) [2007:1 | 2007:4 | 2007:3] -> Set Event Flag(flag, OFF) [2003:66] ->
END Restart [1000:4]. Event 0 gets one Initialize Event(slot 0, id) [2000:0] per event.
OFF-4: instead of a message, an event may carry "bots": <table id> (the game's NPC bot start, 2003:97 with one s32, archive/research/OFF-3-bots.md)
or "bots_clear": true (2003:98, no arguments). Neither is in DarkScript3's NR EMEDF; the argument layout comes from the bank 2003
dispatcher (case 97 reads one s32 at args+0; case 98 reads none).
M5-3 (archive/research/M5-no-boss-nights.md): an event may carry "night_end": 1 | 2 with "secs" and "flag": once the night is on (7515), it is
night 1 (7505 off) or 2 (7505 on), that night is not done yet (7502 / 7507 off) and this game is the host (own world), it waits "secs"
or our "flag", then sets the night-done flag network-wide (and 8020 when the special event 7700 / 7720 is on, as the boss-defeat
handler 90065900 does), clears our flag, and ends (no restart). M5-5: no heal there any more (that event runs only in the host's game);
it ends without setting anything when the night is already done (the DLL's fallback write got there first).
M5-8 / M5-9: "speffect": <SpEffect id> with "flag": a per-game switch; while the flag is ON the local player holds that SpEffect,
when it goes OFF the SpEffect is cleared (102130 = the player script's "die at once, no downed state", archive/research/M5-8-death.md).
M5-5 / M6-1: per-game events (Set Network Sync State Disabled first, as the game's own per-player events do), started by our flag,
which they clear: "heal": true = Bonfire-like Recovery of the local player, then restart (the night winner); "ending": "win" | "lose"
with "banner" (TextEffect param id: 4000 win, 4010 failure) and "secs" = banner, wait, then the local flag 7000 (win) / 7001 (failure),
as the Nightlord maps (x002590) and the wipe check (common 1180) do. It ends.
"replace": [{"id": <existing event>, "body": "end" | "disable_chr", "param": <byte offset X of the character parameter>}] gives an
existing event a new body: "end" = END; "disable_chr" = Change Character Enable State(X, Disabled), Set Character AI State(X, Disabled),
END, with X substituted from the event's parameters like the game's own events do.

Layout (SoulsFormats EMEVD, DS3/ER 64-bit; verified on NR 1.03.3 m10_00_00_00.emevd, see .claude/archive/experiments/UI-0/emevd_dump.py):
header 0x90 | events (0x30 each) | instructions (0x20 each: bank i32, id i32, argsLen i64, argsOff i64 rel. to args, layerOff i64 = -1)
| args blob | parameters (0x20 each: instruction index i64 rel. to the event, target byte i64, source byte i64, byte count i32, 1 i32)
| linked files (8 each) | strings. Event instruction offsets are relative to the instruction table, argument offsets to the args blob,
parameter offsets to the parameter table, so appending to the end of each table keeps every existing offset valid. Event 0's
instruction list and every replaced event's body are written at the end of the table (the old copies stay, unreferenced).
Argument packing (DarkScript3 nr-common.emedf.json, checked against the file): 3:0 = s8 group, u8 state, u8 type, pad, u32 flag;
2003:66 and 2003:69 = u8 type, pad3, u32 flag, u8 state, pad3; 1000:4 = u8 end type, pad3; 2000:0 = s32 slot, u32 event id, u32 param;
2007:1 = s32 msg, s16 type, s16 options, u32 entity, f32 distance; 2007:3 = s32 msg, u8 pad state, pad3; 2007:4 = s32 msg;
0:0 = s8 group, u8 state, s8 target group, pad; 1:0 = s8 group, pad3, f32 seconds; 3:26 = s8 group, u8 world type, pad2;
1003:1 = u8 lines, u8 state, u8 type, pad, u32 flag; 1001:0 = f32 seconds; 2004:8 = s32 entity, s32 SpEffect; 2004:47 = none;
2004:5 and 2004:1 = s32 entity, u8 state, pad3; 1003:2 = u8 end type, u8 state, u8 type, pad, u32 flag; 2000:2 = u8 state, pad3;
2007:19 = s32 TextEffect param id; 2004:10 and 2004:39 = s32 entity, u8 state, pad3 (as 2004:5).
ARENA-6: "char_switch": [[bank, id, state while ON], ...] with "flag" (and "secs"): see char_switch_instrs.
"""
import json, struct, sys

HEADER = 0x90
EV = 0x30
INS = 0x20
PAR = 0x20

MAIN, AND_01, OR_01 = 0, 1, -1
NIGHT = 7515
DAY2 = 7505
NIGHT_DONE = {1: 7502, 2: 7507}
SPECIAL = {1: 7700, 2: 7720}
BOSSES_DONE = 8020


def flag_cond(group, on, flag):
    return (3, 0, struct.pack('<bBBxI', group, 1 if on else 0, 0, flag))


def set_flag(flag, on, network=False):
    return (2003, 69 if network else 66, struct.pack('<B3xIB3x', 0, flag, 1 if on else 0))


def night_end_instrs(ev):
    n, secs, flag = ev['night_end'], float(ev['secs']), ev['flag']
    if n not in NIGHT_DONE:
        raise SystemExit(f"night_end must be 1 or 2, not {n!r}")
    return [
        flag_cond(AND_01, True, NIGHT),
        flag_cond(AND_01, n == 2, DAY2),
        flag_cond(AND_01, False, NIGHT_DONE[n]),
        (3, 26, struct.pack('<bBxx', AND_01, 0)),             # this game is in its own world (host or solo)
        (0, 0, struct.pack('<bBbx', MAIN, 1, AND_01)),
        (1, 0, struct.pack('<b3xf', OR_01, secs)),             # the night lasts secs...
        flag_cond(OR_01, True, flag),                          # ...or until our flag (the DLL: one team left; console night_end)
        (0, 0, struct.pack('<bBbx', MAIN, 1, OR_01)),
        end_if_flag(True, NIGHT_DONE[n]),                      # the DLL's fallback write already ended this night
        set_flag(NIGHT_DONE[n], True, network=True),
        (1003, 1, struct.pack('<BBBxI', 1, 0, 0, SPECIAL[n])),  # skip the next line when the special event is off
        set_flag(BOSSES_DONE, True, network=True),
        set_flag(flag, False),
        (1000, 4, struct.pack('<B3x', 0)),
    ]


def end_if_flag(on, flag):
    return (1003, 2, struct.pack('<BBBxI', 0, 1 if on else 0, 0, flag))


RESULT = {'win': 7000, 'lose': 7001}


def local_instrs(ev):
    """M5-5 / M6-1: per-game events started by our flag."""
    flag = ev['flag']
    out = [(2000, 2, struct.pack('<B3x', 0)), flag_cond(MAIN, True, flag), set_flag(flag, False)]
    if ev.get('heal'):
        return out + [(2004, 47, b''), (1000, 4, struct.pack('<B3x', 1))]
    kind = ev['ending']
    if kind not in RESULT:
        raise SystemExit(f"ending must be win or lose, not {kind!r}")
    return out + [
        (2007, 19, struct.pack('<i', ev['banner'])),
        (1001, 0, struct.pack('<f', float(ev['secs']))),
        set_flag(RESULT[kind], True),
        (1000, 4, struct.pack('<B3x', 0)),
    ]


def speffect_instrs(ev):
    """M5-8 / M5-9: a per-game switch. While our flag is ON the local player (10000) holds SpEffect `speffect` (Set SpEffect 2004:8);
    when it goes OFF the SpEffect is cleared (Clear SpEffect 2004:21); then the event restarts. The DLL owns the flag."""
    flag, sp = ev['flag'], ev['speffect']
    return [
        (2000, 2, struct.pack('<B3x', 0)),
        flag_cond(MAIN, True, flag),
        (2004, 8, struct.pack('<ii', 10000, sp)),
        flag_cond(MAIN, False, flag),
        (2004, 21, struct.pack('<ii', 10000, sp)),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def immortal_instrs(ev):
    """ARENA-HP1: a per-game switch. While our flag is ON the local player (10000) is immortal (Set Character Immortality 2004:12,
    u32 entity, u8 on, pad3: real hits stop at 1 HP, no down); re-applied every `secs` while ON, so a game call that clears it
    (recovery, level change) is undone at once; when the flag goes OFF it is turned off. The DLL owns the flag."""
    flag = ev['flag']
    return [
        (2000, 2, struct.pack('<B3x', 0)),
        flag_cond(MAIN, True, flag),
        (2004, 12, struct.pack('<iB3x', 10000, 1)),
        (1001, 0, struct.pack('<f', float(ev.get('secs', 0.5)))),
        (1003, 2, struct.pack('<BBBxI', 1, 1, 0, flag)),         # still ON: restart (apply again)
        (2004, 12, struct.pack('<iB3x', 10000, 0)),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def char_switch_instrs(ev):
    """ARENA-6: a per-game switch of character states of the local player (10000). `char_switch` lists [bank, id, state while ON];
    each is an instruction with the layout s32 entity, u8 state, pad3 (2004:5 Change Character Enable State, 2004:10 Set Character
    Gravity, 2004:39 Change Character Collision State: 0 Disabled, 1 Enabled). While our flag is ON they are applied and re-applied
    every `secs`; when it goes OFF each gets the other state (1 - state), then the event restarts. The DLL owns the flag."""
    flag, lines = ev['flag'], ev['char_switch']
    out = [(2000, 2, struct.pack('<B3x', 0)), flag_cond(MAIN, True, flag)]
    out += [(b, i, struct.pack('<iB3x', 10000, s)) for b, i, s in lines]
    out += [
        (1001, 0, struct.pack('<f', float(ev.get('secs', 0.5)))),
        (1003, 2, struct.pack('<BBBxI', 1, 1, 0, flag)),         # still ON: restart (apply again)
    ]
    out += [(b, i, struct.pack('<iB3x', 10000, 1 - s)) for b, i, s in lines]
    out.append((1000, 4, struct.pack('<B3x', 1)))
    return out


def mirror_instrs(ev):
    """RW-5 verdict transport: while the local flag `from` is ON the network-connected flag `to` is ON in every game, OFF when it goes
    OFF. One game's DLL sets `from` (a local write there); every game's DLL reads `to`. Then the event restarts."""
    a, b = ev['from'], ev['to']
    return [
        flag_cond(MAIN, True, a),
        set_flag(b, True, network=True),
        flag_cond(MAIN, False, a),
        set_flag(b, False, network=True),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def warp_instrs(ev):
    """ARENA-0: a per-game warp. When our flag goes ON, this game's player is warped to entity `entity` of map `warp`
    ([area, block, region, index], e.g. [18, 0, 0, 0] = m18_00_00_00) with Warp Player 2003:93, the instruction the Spirit Shelter's
    `m12` event 12002099 and `common` 1200 use (args: u8 area, u8 block, u8 region, u8 index, u32 entity). Then the event restarts."""
    flag, (a, b, r, i), entity = ev['flag'], ev['warp'], ev['entity']
    return [
        (2000, 2, struct.pack('<B3x', 0)),
        flag_cond(MAIN, True, flag),
        set_flag(flag, False),
        (2003, 93, struct.pack('<BBBBI', a, b, r, i, entity)),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def camera_instrs(ev):
    """ARENA-1: turn this game's camera. When our flag goes ON: Set Camera Angle 2008:4 (f32 X angle, f32 Y angle, as `m18` event
    18002600 uses right after its short warp), the flag goes OFF, the event restarts. Per game (network sync off)."""
    flag, (x, y) = ev['flag'], ev['camera']
    return [
        (2000, 2, struct.pack('<B3x', 0)),
        flag_cond(MAIN, True, flag),
        set_flag(flag, False),
        (2008, 4, struct.pack('<ff', float(x), float(y))),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def fade_instrs(ev):
    """ARENA-2: when our flag goes ON, Fade to Black 2004:77 for this game's screen (f32 ratio: 1 black, 0 clear; f32 seconds;
    u8 freeze the player; f32 freeze delay, -1 = none; as `m18` 18002500 / 18002601 use it), the flag goes OFF, restart."""
    flag, (ratio, secs, freeze) = ev['flag'], ev['fade']
    return [
        (2000, 2, struct.pack('<B3x', 0)),
        flag_cond(MAIN, True, flag),
        set_flag(flag, False),
        (2004, 77, struct.pack('<ffB3xf', float(ratio), float(secs), 1 if freeze else 0, -1.0)),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def refill_instrs(ev):
    """ARENA-2: when our flag goes ON, Refill Estus 2004:87 (no arguments) for this game's player, the flag goes OFF, restart."""
    flag = ev['flag']
    return [
        (2000, 2, struct.pack('<B3x', 0)),
        flag_cond(MAIN, True, flag),
        set_flag(flag, False),
        (2004, 87, b''),
        (1000, 4, struct.pack('<B3x', 1)),
    ]


def disable_instrs(ev):
    """ARENA-0: keep characters off. Every `secs` the listed entities (characters or character groups) get the same two lines as
    the M5-3 "disable_chr" body (2004:5 and 2004:1 with state 0: the character and its AI disabled), so a game event that enables
    them again is undone within `secs`. Runs as long as the map is loaded."""
    out = []
    for e in ev['disable']:
        out.append((2004, 5, struct.pack('<iB3x', e, 0)))
        out.append((2004, 1, struct.pack('<iB3x', e, 0)))
    out.append((1001, 0, struct.pack('<f', float(ev.get('secs', 1.0)))))
    out.append((1000, 4, struct.pack('<B3x', 1)))
    return out


def build_event_instrs(ev):
    if 'disable' in ev:
        return disable_instrs(ev)
    if 'warp' in ev:
        return warp_instrs(ev)
    if 'camera' in ev:
        return camera_instrs(ev)
    if ev.get('refill_estus'):
        return refill_instrs(ev)
    if 'fade' in ev:
        return fade_instrs(ev)
    if 'mirror' in ev:
        return mirror_instrs(ev)
    if 'night_end' in ev:
        return night_end_instrs(ev)
    if 'speffect' in ev:
        return speffect_instrs(ev)
    if ev.get('immortal'):
        return immortal_instrs(ev)
    if 'char_switch' in ev:
        return char_switch_instrs(ev)
    if ev.get('heal') or 'ending' in ev:
        return local_instrs(ev)
    flag, msg, kind = ev['flag'], ev.get('message'), ev.get('display')
    out = [(3, 0, struct.pack('<bBBxI', 0, 1, 0, flag))]
    if kind is not None:
        # FS7-1: a message is for this game's player only. Without Set Network Sync State Disabled (2000:2) a synced event that fires
        # in one game fires in every game of the session, and each one shows ITS OWN copy of the text (friend session 2026-11-07:
        # the spectator hint and the own K/D showed on every screen, with the other game's stale numbers).
        out.insert(0, (2000, 2, struct.pack('<B3x', 0)))
    if 'bots' in ev:
        out.append((2003, 97, struct.pack('<i', ev['bots'])))
    elif ev.get('bots_clear'):
        out.append((2003, 98, b''))
    elif kind == 'dialog':
        out.append((2007, 1, struct.pack('<ihhIf', msg, 0, 0, 0, 5.0)))
    elif kind == 'blinking':
        out.append((2007, 4, struct.pack('<i', msg)))
    elif kind == 'status':
        out.append((2007, 3, struct.pack('<iB3x', msg, 0)))
    else:
        raise SystemExit(f"unknown display {kind!r}")
    out.append((2003, 66, struct.pack('<B3xIB3x', 0, flag, 0)))
    out.append((1000, 4, struct.pack('<B3x', 1)))
    return out


def replacement(r):
    """(instructions, parameters) of a replaced event body; a parameter is (instruction index, target byte, source byte, count)."""
    if r['body'] == 'end':
        return [(1000, 4, struct.pack('<B3x', 0))], []
    if r['body'] == 'disable_chr':
        x = r['param']
        ins = [(2004, 5, struct.pack('<iB3x', 0, 0)), (2004, 1, struct.pack('<iB3x', 0, 0)), (1000, 4, struct.pack('<B3x', 0))]
        return ins, [(0, 0, x, 4), (1, 0, x, 4)]
    raise SystemExit(f"unknown replacement body {r['body']!r}")


def main(path, spec_path):
    b = bytearray(open(path, 'rb').read())
    if b[:4] != b'EVD\0' or b[4:8] != bytes.fromhex('00ff01ff'):
        raise SystemExit('not an ER/NR 64-bit EMEVD')
    q = lambda o: struct.unpack_from('<q', b, o)[0]
    h = {n: q(o) for n, o in [('eventCount', 0x10), ('eventsOff', 0x18), ('instrCount', 0x20), ('instrOff', 0x28),
                                ('unkCount', 0x30), ('unkOff', 0x38), ('layerCount', 0x40), ('layersOff', 0x48),
                                ('paramCount', 0x50), ('paramsOff', 0x58), ('linkedCount', 0x60), ('linkedOff', 0x68),
                                ('argsLen', 0x70), ('argsOff', 0x78), ('stringsLen', 0x80), ('stringsOff', 0x88)]}
    if h['unkCount'] or h['layerCount']:
        raise SystemExit('layers/unk tables not supported')
    if not (h['eventsOff'] == HEADER and h['instrOff'] == HEADER + EV * h['eventCount']
            and h['argsOff'] == h['instrOff'] + INS * h['instrCount'] and h['paramsOff'] == h['argsOff'] + h['argsLen']
            and h['linkedOff'] == h['paramsOff'] + PAR * h['paramCount'] and h['stringsOff'] == h['linkedOff'] + 8 * h['linkedCount']):
        raise SystemExit('unexpected section layout')
    whole = json.load(open(spec_path, encoding='utf-8'))
    spec = whole.get('events', [])
    replace = whole.get('replace', [])

    events = [bytearray(b[h['eventsOff'] + i * EV: h['eventsOff'] + (i + 1) * EV]) for i in range(h['eventCount'])]
    index = {struct.unpack_from('<q', e, 0)[0]: i for i, e in enumerate(events)}
    instrs = [bytearray(b[h['instrOff'] + i * INS: h['instrOff'] + (i + 1) * INS]) for i in range(h['instrCount'])]
    args = bytearray(b[h['argsOff']: h['argsOff'] + h['argsLen']])
    params = bytearray(b[h['paramsOff']: h['linkedOff']])
    tail = b[h['linkedOff']: h['stringsOff'] + h['stringsLen']]  # linked files + strings, copied as is

    def add_instr(bank, iid, a):
        if not a:  # no arguments: length 0, offset -1 (as the game's own argument-less instructions)
            instrs.append(bytearray(struct.pack('<iiqqq', bank, iid, 0, -1, -1)))
            return len(instrs) - 1
        while len(args) % 4:
            args.append(0)
        off = len(args)
        args.extend(a)
        instrs.append(bytearray(struct.pack('<iiqqq', bank, iid, len(a), off, -1)))
        return len(instrs) - 1

    # Replaced bodies (M5-3): new instructions and parameter entries at the end; the event keeps its id and rest behaviour.
    for r in replace:
        if r['id'] not in index:
            raise SystemExit(f"event {r['id']} to replace does not exist")
        i = index[r['id']]
        eid, ic, io, pc, po, rb, pad = struct.unpack('<qqqqqII', events[i])
        lines, pars = replacement(r)
        first = len(instrs)
        for bank, iid, a in lines:
            add_instr(bank, iid, a)
        poff = len(params)
        for k, tgt, src, n in pars:
            if src + n > 4 * 32:
                raise SystemExit(f"parameter X{src} out of range")
            params.extend(struct.pack('<qqqii', k, tgt, src, n, 1))
        events[i] = bytearray(struct.pack('<qqqqqII', eid, len(lines), first * INS, len(pars), poff if pars else -1, rb, pad))

    # Our events.
    for ev in spec:
        if ev['id'] in index:
            raise SystemExit(f"event id {ev['id']} already exists")
        first = len(instrs)
        lines = build_event_instrs(ev)
        for bank, iid, a in lines:
            add_instr(bank, iid, a)
        events.append(bytearray(struct.pack('<qqqqqII', ev['id'], len(lines), first * INS, 0, -1, 0, 0)))

    # Event 0: copy its list to the end, add our Initialize Event lines.
    if spec:
        e0 = events[0]
        eid, ic, io, pc, po, rb, pad = struct.unpack('<qqqqqII', e0)
        if eid != 0 or pc != 0:
            raise SystemExit('event 0 missing or has parameters')
        first = len(instrs)
        for k in range(io // INS, io // INS + ic):
            instrs.append(bytearray(instrs[k]))
        for ev in spec:
            add_instr(2000, 0, struct.pack('<iII', 0, ev['id'], 0))
        events[0] = bytearray(struct.pack('<qqqqqII', 0, ic + len(spec), first * INS, 0, -1, rb, pad))

    while len(args) % 16:
        args.append(0)
    n_ev, n_in, n_par = len(events), len(instrs), len(params) // PAR
    events_off = HEADER
    instr_off = events_off + EV * n_ev
    args_off = instr_off + INS * n_in
    params_off = args_off + len(args)
    linked_off = params_off + len(params)
    strings_off = linked_off + 8 * h['linkedCount']
    out = bytearray(b[:HEADER])
    for e in events:
        out += e
    for i in instrs:
        out += i
    out += args
    out += params
    out += tail
    struct.pack_into('<I', out, 12, len(out))
    for name, val in [(0x10, n_ev), (0x18, events_off), (0x20, n_in), (0x28, instr_off), (0x38, args_off), (0x48, args_off),
                      (0x50, n_par), (0x58, params_off), (0x68, linked_off), (0x70, len(args)), (0x78, args_off), (0x88, strings_off)]:
        struct.pack_into('<q', out, name, val)
    open(path, 'wb').write(out)
    print(f'{path}: +{len(spec)} events (now {n_ev}), {len(replace)} replaced, +{n_in - h["instrCount"]} instructions, '
          f'+{n_par - h["paramCount"]} parameters, {len(b)} -> {len(out)} bytes')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
