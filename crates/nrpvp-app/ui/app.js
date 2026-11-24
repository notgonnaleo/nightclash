// Nightclash launcher UI. Talks to the Rust side (src/main.rs) through Tauri commands; the Rust side never takes a URL or a
// path to open from here except paths the player picked in a dialog. Opened in a plain browser (no Tauri), it runs on a mock so the
// look can be checked without building.
'use strict';

const T = window.__TAURI__;
const invoke = T ? T.core.invoke : mockInvoke;
const listen = T ? T.event.listen : () => {};
const appWindow = T ? T.window.getCurrentWindow() : null;
const $ = (s) => document.querySelector(s);
const $$ = (s) => [...document.querySelectorAll(s)];

const S = {
  status: null,        // Status from Rust
  release: null,       // latest Manifest
  releaseError: null,
  exeHash: null,
  scGithub: undefined, // undefined = not asked yet, null = none
  busy: null,          // label of the running job
  roomDirty: false,    // the private room field holds an unsaved edit
  step: 0,
};

// ---------- helpers ----------
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const mb = (n) => (n / 1048576).toFixed(n > 10485760 ? 0 : 1) + ' MB';
const same = (a, b) => !!a && !!b && a.toUpperCase() === b.toUpperCase();
const newer = (a, b) => {
  if (!/^\d+\.\d+\.\d+$/.test(a || '') || !/^\d+\.\d+\.\d+$/.test(b || '')) return false;
  const x = a.split('.').map(Number), y = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i];
  return false;
};
let toastTimer;
function toast(msg, ok = false) {
  const t = $('#toast');
  t.textContent = msg;
  t.className = 'toast' + (ok ? ' ok' : '');
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), ok ? 3500 : 9000);
}
function notesHtml(text) {
  if (!text || !text.trim()) return '<p class="muted">No notes for this release.</p>';
  return text.trim().split(/\n\s*\n/).map((block) => {
    const lines = block.split('\n');
    if (lines.every((l) => /^\s*[-*] /.test(l))) return '<ul>' + lines.map((l) => '<li>' + esc(l.replace(/^\s*[-*] /, '')) + '</li>').join('') + '</ul>';
    return '<p>' + lines.map(esc).join('<br>') + '</p>';
  }).join('');
}

// ---------- derived state ----------
function derive() {
  const st = S.status || {};
  const m = st.mod || null;
  const r = S.release;
  const d = {
    gameOk: !!st.game_dir,
    scOk: !!st.sc_installed,
    scMatch: r && st.sc_sha256 ? same(r.seamless_coop.nrsc_sha256, st.sc_sha256) : null,
    modOk: !!(m && m.present && !m.problem),
    modVersion: m && m.version,
    localBuild: !!(m && m.local_build),
    gameMatch: r && S.exeHash ? same(r.game.exe_sha256, S.exeHash) : null,
  };
  // LAUNCH-GATE: an install older than the latest release (or with no version) cannot be played; the Rust side refuses it too.
  d.update = !!(r && d.modOk && !d.localBuild && (!d.modVersion || newer(r.version, d.modVersion)));
  // LAUNCH-GATE: no Play until the latest version is known (the Rust side refuses too); a local test build needs no check.
  d.checking = !r && !S.releaseError && !d.localBuild;
  d.noCheck = !r && !!S.releaseError && !d.localBuild;
  d.newLauncher = !!(r && r.launcher && newer(r.version, st.launcher_version) && st.launcher_sha256 && !same(r.launcher.sha256, st.launcher_sha256));
  d.ready = d.gameOk && d.scOk && d.modOk;
  return d;
}

// ---------- render ----------
function render() {
  const d = derive();
  const st = S.status || {};
  const r = S.release;

  // chips
  const chips = [];
  chips.push('<span class="chip"><i></i>Player versus player</span>');
  if (st.private_code) chips.push(`<span class="chip room"><i></i>Private room&nbsp;<b>${esc(st.private_code)}</b></span>`);
  else chips.push('<span class="chip"><i></i>Online matchmaking</span>');
  if (r) chips.push(`<span class="chip"><i></i>Game ${esc(r.game.version)}</span>`);
  if (d.gameMatch === false) chips.push('<span class="chip warn"><i></i>Game updated since this release</span>');
  $('#chips').innerHTML = chips.join('');

  // private room (RW-6)
  const code = st.private_code || '';
  const rs = $('#roomState');
  rs.className = 'room-state' + (code ? ' on' : '');
  rs.innerHTML = code
    ? `<i></i><span>Private room <code>${esc(code)}</code>: you meet only players with this code.</span>`
    : '<i></i><span>Public matchmaking: you meet anyone who is searching.</span>';
  if (document.activeElement !== $('#roomCode') && !S.roomDirty) $('#roomCode').value = code;
  $('#roomClear').hidden = !code;
  const roomBusy = !st.mod || !st.mod.present || !!S.busy;
  $('#roomSave').disabled = roomBusy;
  $('#roomCode').disabled = roomBusy;

  // notes
  if (r) {
    $('#notesVer').textContent = 'v' + r.version;
    $('#notesDate').textContent = r.published;
    $('#notes').innerHTML = notesHtml(r.notes);
  } else if (S.releaseError) {
    $('#notesVer').textContent = '—';
    $('#notesDate').textContent = '';
    $('#notes').innerHTML = `<p class="muted">${esc(S.releaseError)}</p>`;
  }

  // install checks
  const row = (state, title, sub) => `<li><span class="mark ${state}"></span><span><b>${esc(title)}</b><small>${esc(sub)}</small></span></li>`;
  $('#checks').innerHTML = [
    row(d.gameOk ? (d.gameMatch === false ? 'warn' : 'ok') : 'bad', 'ELDEN RING NIGHTREIGN',
      d.gameOk ? st.game_dir + (d.gameMatch === false ? ' — a different game version than this release was built for' : '') : 'Not found'),
    row(d.scOk ? (d.scMatch === false ? 'warn' : 'ok') : 'bad', 'Seamless Co-op',
      d.scOk ? (d.scMatch === false ? `Installed, but not v${r.seamless_coop.version} (the version this release needs)` : `Installed${r ? ' · v' + r.seamless_coop.version : ''}`) : 'Not installed'),
    row(d.modOk ? (d.update ? 'warn' : 'ok') : 'bad', 'Nightclash',
      d.modOk ? (d.modVersion ? 'v' + d.modVersion : d.localBuild ? 'Local test build' : 'Installed by hand (unknown version)') + (d.update ? ` · v${r.version} required` : '') : (st.mod && st.mod.present ? st.mod.problem : 'Not installed')),
  ].join('') + (d.newLauncher ? row('warn', 'New launcher available', 'Restart the launcher to retry the automatic update, or download it from Settings › All releases.') : '')
    + (st.foreign && st.foreign.length ? row('bad', 'Files from another tool in the game folder',
      `${st.foreign.join(', ')}: they make the game load other code and break matches. Move them out of the game folder, then use Steam's "Verify integrity of game files".`) : '');

  // dock
  const dot = $('#dockDot'), text = $('#dockText'), sub = $('#dockSub'), btn = $('#btnPrimary'), label = $('#primaryLabel');
  btn.classList.toggle('busy', !!S.busy);
  if (S.busy) {
    dot.className = 'dot busy'; text.textContent = S.busy; btn.disabled = true; label.textContent = 'Working';
  } else if (!S.status) {
    dot.className = 'dot busy'; text.textContent = 'Checking your install…'; btn.disabled = true; label.textContent = '…';
  } else if (!d.gameOk || !d.scOk) {
    dot.className = 'dot bad'; text.textContent = !d.gameOk ? 'Game folder not found' : 'Seamless Co-op is not installed';
    sub.textContent = 'Setup walks you through it.'; btn.disabled = false; label.textContent = 'Set up';
  } else if (!d.modOk) {
    dot.className = 'dot bad'; text.textContent = st.mod && st.mod.present ? 'The mod install is incomplete' : 'Nightclash is not installed';
    sub.textContent = r ? `v${r.version} · ${mb(r.zip.size)}` : (S.releaseError || 'Looking for the latest release…');
    btn.disabled = !r; label.textContent = st.mod && st.mod.present ? 'Repair' : 'Install';
  } else if (d.update) {
    dot.className = 'dot bad'; text.textContent = `Update required: v${r.version}`;
    sub.textContent = `This version cannot be played: everyone in a match needs v${r.version}.`; btn.disabled = false; label.textContent = 'Update';
  } else if (d.checking) {
    dot.className = 'dot busy'; text.textContent = 'Checking for updates…'; sub.textContent = ''; btn.disabled = true; label.textContent = 'Play';
  } else if (d.noCheck) {
    dot.className = 'dot bad'; text.textContent = 'Could not check for updates';
    sub.textContent = `${S.releaseError} The game starts only after this check.`; btn.disabled = false; label.textContent = 'Retry';
  } else if (st.foreign && st.foreign.length && !st.game_running) {
    dot.className = 'dot bad'; text.textContent = 'Files from another tool in the game folder';
    sub.textContent = `${st.foreign.join(', ')}: matches will fail. Install tab › how to fix.`; btn.disabled = false; label.textContent = 'Play';
  } else {
    dot.className = 'dot ok';
    text.textContent = st.game_running ? 'The game is running' : 'Ready to play';
    sub.textContent = [d.modVersion ? 'Nightclash v' + d.modVersion : 'Nightclash', r ? 'Seamless Co-op v' + r.seamless_coop.version : 'Seamless Co-op']
      .join(' · ') + (d.gameMatch === false ? ' · the game was updated: the mod may not work until a new release' : '') + (S.releaseError ? ' · offline' : '');
    btn.disabled = !!st.game_running; label.textContent = 'Play';
  }
  if (!S.busy) $('#dockBarWrap').hidden = true;

  // settings drawer
  $('#setGame').textContent = st.game_dir || 'Not found';
  $('#setMod').textContent = d.modOk ? (d.modVersion ? 'v' + d.modVersion : d.localBuild ? 'local test build' : 'installed by hand') : 'Not installed';
  $('#setSc').textContent = d.scOk ? (d.scMatch === false ? 'Installed (a different version than this release needs)' : 'Installed') : 'Not installed';
  $('#setVer').textContent = st.launcher_version ? 'Launcher v' + st.launcher_version + (d.newLauncher ? ' · a newer one is out' : '') : '';
  $('#setRepair').disabled = !d.gameOk || !r || !!S.busy;
  const player = st.player || {}, characters = player.characters || [];
  $('#playerSteamId').textContent = player.steam_id || 'Start Steam and sign in. No manual SteamID is needed.';
  const playerMessage = player.problem || (player.present ? 'Your Seamless Co-op save is installed.' : 'The bundled starter save will be installed during setup.');
  $('#playerStatus').textContent = playerMessage;
  $('#wzPlayerStatus').textContent = playerMessage;
  const options = characters.map(c => `<option value="${c.slot}">Slot ${c.slot + 1}: ${esc(c.name)}</option>`).join('');
  if ($('#playerSlot').innerHTML !== options) $('#playerSlot').innerHTML = options;
  $('#playerSave').disabled = !!S.busy || !!st.game_running || !characters.length || !!player.problem;
  $('#playerSetup').disabled = !!S.busy || !!st.game_running || !d.scOk || !player.steam_id;
  $('#playerReplace').disabled = !!S.busy || !!st.game_running || !d.scOk || !player.steam_id;
  const steamSave = player.steam_save || {};
  $('#playerImportSteam').disabled = !!S.busy || !!st.game_running || !d.scOk || !player.steam_id || !steamSave.present || !!steamSave.problem;
  $('#steamSaveStatus').textContent = steamSave.problem || (steamSave.present
    ? 'Local Steam save found' + ((steamSave.characters || []).length ? ': ' + steamSave.characters.map(c => c.name).join(', ') : '') + '. Imports all progress and original character names.'
    : 'No local Steam save found. Start the normal game and let Steam sync your save.');

  if (!$('#wizard').hidden) renderWizard();
}

// ---------- wizard ----------
function renderWizard() {
  const d = derive();
  const st = S.status || {};
  const r = S.release;
  $$('#steps li').forEach((li) => {
    const i = +li.dataset.step;
    li.className = i === S.step ? 'current' : i < S.step ? 'done' : '';
  });
  $$('.wz-page').forEach((p) => (p.hidden = +p.dataset.page !== S.step));
  const next = $('#wzNext');
  $('#wzBack').style.visibility = S.step > 0 && S.step < 3 ? 'visible' : 'hidden';
  next.disabled = !!S.busy;

  if (S.step === 0) {
    $('#wzGame').innerHTML = d.gameOk
      ? `<span class="mark ok"></span><span><b>Found the game</b><small>${esc(st.game_dir)}</small></span>`
      : `<span class="mark bad"></span><span><b>Not found automatically</b><small>Choose the folder that holds nightreign.exe.</small></span>`;
    next.textContent = 'Continue';
    next.disabled = !d.gameOk || !!S.busy;
  }

  if (S.step === 1) {
    let h = '';
    if (d.scOk) {
      h += `<div class="field"><span class="mark ${d.scMatch === false ? 'warn' : 'ok'}"></span><span><b>Seamless Co-op is installed</b><small>${esc(st.game_dir)}\\SeamlessCoop</small></span></div>`;
      if (d.scMatch === false) {
        h += `<div class="card warn"><h4>A different version than this release needs</h4><p class="muted" style="margin:0">This release was built and tested with Seamless Co-op <b>v${esc(r.seamless_coop.version)}</b>. Everyone in a match must have the same version. If matches fail, install v${esc(r.seamless_coop.version)} from Nexus Mods (Files &rsaquo; older versions) below.</p></div>`;
      }
    }
    if (!d.scOk || d.scMatch === false) {
      if (S.scGithub) {
        h += `<div class="card"><h4>From Yui's GitHub</h4><p class="muted" style="margin:0 0 4px">${esc(S.scGithub.name)} · ${esc(S.scGithub.tag)} · ${mb(S.scGithub.size)}</p>
              <div class="row"><button class="btn gold" id="scGh">Download and install</button></div></div>`;
      }
      h += `<div class="card"><h4>${S.scGithub ? 'Or from' : 'From'} Nexus Mods (the official page)</h4>
            <ol>
              <li>Open the page and download the <b>Main file</b>${r ? ` (v${esc(r.seamless_coop.version)})` : ''}. Nexus needs a free account.</li>
              <li>Come back here. The launcher finds the zip in your Downloads folder and installs it.</li>
            </ol>
            <div class="row">
              <button class="btn" data-open="nexus">Open Nexus Mods</button>
              <button class="btn gold" id="scFind">Find it in Downloads</button>
              <button class="btn" id="scPick">Choose zip…</button>
            </div></div>`;
      h += `<p class="hint">Your Seamless Co-op settings (<code>nrsc_settings.ini</code>) are kept when you reinstall.</p>`;
    }
    $('#wzSc').innerHTML = h;
    bindOpen($('#wzSc'));
    $('#scGh') && ($('#scGh').onclick = () => job('Installing Seamless Co-op…', () => invoke('install_sc_github', { rel: S.scGithub }), 'Seamless Co-op installed.'));
    $('#scFind') && ($('#scFind').onclick = findScZip);
    $('#scPick') && ($('#scPick').onclick = pickScZip);
    $$('#wzSc button').forEach((b) => (b.disabled = !!S.busy));
    next.textContent = 'Continue';
    next.disabled = !d.scOk || !!S.busy;
  }

  if (S.step === 2) {
    let h = '';
    if (r) {
      h += `<div class="release-line"><span class="v">v${esc(r.version)}</span><span class="meta">${esc(r.published)} · ${mb(r.zip.size)}${r.match_players ? ` · ${r.match_players} players per match` : ''}</span></div>`;
      h += `<div class="card notes" style="max-height:170px;overflow:auto">${notesHtml(r.notes)}</div>`;
    } else {
      h += `<div class="card warn"><h4>No release found</h4><p class="muted" style="margin:0">${esc(S.releaseError || 'Looking…')}</p><div class="row"><button class="btn" id="wzRetry">Try again</button></div></div>`;
    }
    if (d.modOk && !d.update) h = `<div class="field"><span class="mark ok"></span><span><b>Nightclash is installed</b><small>${d.modVersion ? 'v' + esc(d.modVersion) : 'installed by hand'}</small></span></div>` + h;
    $('#wzMod').innerHTML = h;
    $('#wzRetry') && ($('#wzRetry').onclick = loadRelease);
    if (d.modOk && !d.update) { next.textContent = 'Continue'; next.disabled = !!S.busy; }
    else { next.textContent = d.modOk ? 'Update' : 'Install'; next.disabled = !r || !!S.busy; }
  }

  if (S.step === 3) {
    next.textContent = 'Play now';
    next.disabled = !d.ready || d.checking || d.noCheck || !!S.busy || !!st.game_running;
  }
}

function openWizard(step) {
  const d = derive();
  S.step = step ?? (!d.gameOk ? 0 : !d.scOk ? 1 : !d.modOk || d.update ? 2 : 3);
  $('#wizard').hidden = false;
  if (S.scGithub === undefined) {
    invoke('sc_github').then((x) => { S.scGithub = x; render(); }).catch(() => { S.scGithub = null; });
  }
  render();
}

async function wizardNext() {
  const d = derive();
  if (S.step === 2 && (!d.modOk || d.update)) {
    const ok = await installMod();
    if (!ok) return;
  }
  if (S.step === 3) {
    if (!await setupPlayer($('#wzPlayerName').value)) return;
    return play();
  }
  S.step = Math.min(3, S.step + 1);
  render();
}

// ---------- jobs ----------
async function job(label, fn, okMsg) {
  if (S.busy) return false;
  S.busy = label;
  setBar(0);
  $('#dockBarWrap').hidden = false;
  $('#wzBarWrap').hidden = S.step !== 2;
  render();
  try {
    await fn();
    if (okMsg) toast(okMsg, true);
    return true;
  } catch (e) {
    toast(String(e));
    return false;
  } finally {
    S.busy = null;
    $('#wzBarWrap').hidden = true;
    $('#wzBarText').textContent = '';
    await refresh();
  }
}
function setBar(frac) { $$('.js-bar').forEach((b) => (b.style.width = Math.round(frac * 1000) / 10 + '%')); }

const installMod = () => job('Installing Nightclash…', () => invoke('install_mod'), 'Nightclash is installed.');

async function findScZip() {
  const path = await invoke('find_sc_zip').catch((e) => { toast(String(e)); return null; });
  if (!path) return toast('No Seamless Co-op zip in your Downloads folder yet. Download the Main file from Nexus Mods first, or use "Choose zip…".');
  await job('Installing Seamless Co-op…', () => invoke('install_sc_zip', { path }), 'Seamless Co-op installed from ' + path.split('\\').pop());
}
async function pickScZip() {
  const path = await invoke('pick_sc_zip').catch((e) => { toast(String(e)); return null; });
  if (path) await job('Installing Seamless Co-op…', () => invoke('install_sc_zip', { path }), 'Seamless Co-op installed.');
}
async function pickGame() {
  try { if (await invoke('pick_game_dir')) await refresh(); } catch (e) { toast(String(e)); }
}

async function play() {
  if (S.busy) return;
  S.busy = 'Starting the game…';
  $('#wizard').hidden = true;
  render();
  try {
    await invoke('play'); // the launcher closes itself once the game is starting
  } catch (e) {
    // The version gate may have refused an old install the page still showed as current: reload the release so the dock shows
    // "Update required".
    toast(String(e));
    S.busy = null;
    refresh();
    loadRelease();
  }
}

function primary() {
  const d = derive();
  if (!d.gameOk || !d.scOk) return openWizard();
  if (!d.modOk || d.update) return installMod();
  if (d.noCheck) return loadRelease();
  if (d.checking) return;
  return play();
}

// ---------- data ----------
async function refresh() {
  try { S.status = await invoke('status'); } catch (e) { toast(String(e)); }
  render();
  if (S.status && S.status.game_dir) invoke('game_exe_sha256').then((h) => { S.exeHash = h; render(); }).catch(() => {});
}
async function loadRelease() {
  if (S.releaseError) { S.releaseError = null; render(); } // a retry shows "Checking for updates…"
  try { S.release = await invoke('latest_release'); S.releaseError = null; } catch (e) { S.releaseError = String(e); }
  render();
}

function bindOpen(root) {
  root.querySelectorAll('[data-open]').forEach((b) => (b.onclick = () => invoke('open', { target: b.dataset.open }).catch((e) => toast(String(e)))));
}

// ---------- wiring ----------
$('#btnMin').onclick = () => appWindow && appWindow.minimize();
$('#btnClose').onclick = () => appWindow ? appWindow.close() : null;
$('#btnSettings').onclick = () => { $('#settings').hidden = false; render(); };
$('#btnCloseSettings').onclick = () => ($('#settings').hidden = true);
$('#settings').onclick = (e) => { if (e.target.id === 'settings') $('#settings').hidden = true; };
$('#btnPrimary').onclick = primary;
$('#btnRerunSetup').onclick = () => openWizard(0);
$('#wzNext').onclick = wizardNext;
$('#wzBack').onclick = () => { S.step = Math.max(0, S.step - 1); render(); };
$('#wzLater').onclick = () => { $('#wizard').hidden = true; render(); };
$('#wzPickGame').onclick = pickGame;
$('#setPickGame').onclick = pickGame;
$('#setRepair').onclick = () => { $('#settings').hidden = true; installMod(); };
$('#setScSetup').onclick = () => { $('#settings').hidden = true; openWizard(1); };
async function setupPlayer(name = '') {
  return job('Setting up your save…', async () => {
    await invoke('setup_player');
    if (name.trim()) {
      const status = await invoke('status');
      const character = status.player.characters[0];
      if (!character) throw 'No supported character was found in the save.';
      await invoke('rename_player', { slot: character.slot, name, steamId: status.player.steam_id });
    }
  }, 'Player setup complete.');
}
$('#playerSetup').onclick = () => setupPlayer();
$('#playerRefresh').onclick = refresh;
async function confirmSaveReplacement(source) {
  await refresh();
  if (!S.status?.player?.steam_id || S.status.game_running || S.busy) return;
  if (source === 'steam' && (!S.status.player.steam_save?.present || S.status.player.steam_save.problem)) return;
  S.saveSource = source;
  S.resetAccount = S.status.player.steam_id;
  $('#saveConfirmTitle').textContent = source === 'steam' ? 'Import your Steam save?' : 'Use the all-unlocked save?';
  $('#saveConfirmDescription').textContent = source === 'steam'
    ? 'Your local Steam save will be copied with all its progress and original character names. Your current Seamless Co-op save is backed up before replacement. Your vanilla Steam save is kept unchanged.'
    : 'Your current Seamless Co-op progress will be replaced. A backup is saved beside your save first. Your character name is kept when it can be read.';
  $('#saveConfirmAccount').textContent = `Steam account: ${S.resetAccount}`;
  $('#saveConfirmCheck').checked = false;
  $('#saveConfirmApply').disabled = true;
  $('#saveConfirm').hidden = false;
}
$('#playerReplace').onclick = () => confirmSaveReplacement('starter');
$('#playerImportSteam').onclick = () => confirmSaveReplacement('steam');
$('#saveConfirmCheck').onchange = () => { $('#saveConfirmApply').disabled = !$('#saveConfirmCheck').checked; };
$('#saveConfirmCancel').onclick = () => { $('#saveConfirm').hidden = true; };
$('#saveConfirmApply').onclick = async () => {
  if (!$('#saveConfirmCheck').checked || S.busy) return;
  $('#saveConfirm').hidden = true;
  if (S.saveSource === 'steam') {
    await job('Importing your Steam save…', async () => {
      const backedUp = await invoke('import_steam_save', { steamId: S.resetAccount });
      toast('Steam save imported with its original names.' + (backedUp ? ' Your previous mod save was backed up.' : ''), true);
    });
    return;
  }
  await job('Installing the all-unlocked save…', () => invoke('replace_starter_save', { steamId: S.resetAccount }), 'All-unlocked save installed. Your previous save was backed up.');
};
$('#playerForm').onsubmit = e => {
  e.preventDefault();
  job('Saving your name…', () => invoke('rename_player', { slot: Number($('#playerSlot').value), name: $('#playerName').value, steamId: S.status.player.steam_id }), 'Your in-game name was saved. A backup was kept.');
};
$$('.tab').forEach((t) => (t.onclick = () => {
  $$('.tab').forEach((x) => x.classList.toggle('active', x === t));
  $$('.tab-body').forEach((b) => (b.hidden = b.id !== 'tab-' + t.dataset.tab));
}));
async function saveRoom(code) {
  try {
    const saved = await invoke('set_private_code', { code });
    S.roomDirty = false;
    $('#roomCode').value = saved;
    toast(saved ? `Private room "${saved}" saved. Your friends need the same code.` : 'Back to public matchmaking.', true);
  } catch (e) { toast(String(e)); }
  refresh();
}
$('#roomCode').oninput = () => (S.roomDirty = true);
$('#roomForm').onsubmit = (e) => { e.preventDefault(); saveRoom($('#roomCode').value); };
$('#roomClear').onclick = () => saveRoom('');
bindOpen(document);
document.addEventListener('contextmenu', (e) => e.preventDefault());
window.addEventListener('focus', () => { if (!S.busy) refresh(); });

listen('progress', (e) => {
  const { label, done, total } = e.payload;
  S.busy = label + '…';
  setBar(total ? done / total : 0);
  $('#dockText').textContent = S.busy;
  $('#dockSub').textContent = total > 4096 ? `${mb(done)} of ${mb(total)}` : `${done} of ${total} files`;
  $('#wzBarText').textContent = $('#dockText').textContent + ' ' + $('#dockSub').textContent;
});

(async function start() {
  await Promise.all([refresh(), loadRelease()]);
  let d = derive();
  if (S.status && S.status.auto_update && !S.status.game_running && S.release) {
    if (d.newLauncher) {
      const updating = await job('Updating the launcher…', () => invoke('update_launcher'));
      if (updating) return;
    }
    if (d.update) await installMod();
    d = derive();
  }
  if (!d.gameOk || !d.scOk || !d.modOk) openWizard();
})();

// ---------- embers (decoration) ----------
(function embers() {
  const c = $('#embers'), g = c.getContext('2d');
  let w, h;
  const size = () => { w = c.width = innerWidth; h = c.height = innerHeight; };
  size(); addEventListener('resize', size);
  const P = Array.from({ length: 46 }, () => spawn(true));
  function spawn(any) {
    return { x: Math.random() * w, y: any ? Math.random() * h : h + 10, r: Math.random() * 1.6 + .4, vy: Math.random() * .35 + .12,
      vx: (Math.random() - .5) * .15, a: Math.random() * .6 + .2, t: Math.random() * 6.28 };
  }
  (function tick() {
    g.clearRect(0, 0, w, h);
    for (const p of P) {
      p.y -= p.vy; p.t += .02; p.x += p.vx + Math.sin(p.t) * .2;
      if (p.y < -10) Object.assign(p, spawn(false));
      const fade = Math.min(1, p.y / (h * .5));
      g.beginPath(); g.arc(p.x, p.y, p.r, 0, 6.283);
      g.fillStyle = `rgba(234,${180 + (p.r * 20 | 0)},120,${p.a * fade})`;
      g.shadowColor = 'rgba(214,184,120,.8)'; g.shadowBlur = 6;
      g.fill();
    }
    requestAnimationFrame(tick);
  })();
})();

// ---------- browser mock (no Tauri) ----------
var mockRoom, mockPlayer; // `var`: the mock runs before this line (start() above).
function mockInvoke(cmd, args) {
  const wait = (ms, v) => new Promise((r) => setTimeout(() => r(v), ms));
  const q = new URLSearchParams(location.search);
  const fresh = q.has('fresh');
  switch (cmd) {
    case 'status': return wait(120, { launcher_version: '0.1.0', game_dir: fresh ? null : 'Z:\\SteamLibrary\\steamapps\\common\\ELDEN RING NIGHTREIGN\\Game',
      sc_installed: !fresh, sc_sha256: '243EEC929A97B71E1E2E3B4215778B89C37D629436B8DD5403E830593D3CE24E', sc_launcher: true,
      mod: fresh ? null : { version: q.has('update') ? '0.1.0' : '0.2.0', local_build: false, problem: null, present: true }, game_running: false,
      foreign: q.has('foreign') ? ['OnlineFix.ini', 'dlllist.txt', 'winmm.dll'] : [], private_code: (mockRoom ??= q.get('room') || ''),
      auto_update: false, player: (mockPlayer ??= { steam_id: '76561198000000000', present: !fresh, characters: fresh ? [] : [{ slot: 0, name: 'Nightfarer' }], problem: null }) });
    case 'latest_release': if (q.has('offline')) return Promise.reject('Could not reach GitHub. Check your connection and try again.');
      return wait(300, { schema: 1, version: '0.2.0', tag: 'v0.2.0', published: '2026-11-06', match_players: 2,
      notes: 'The first build for players outside the test group.\n\n- Battle Royale card in the Roundtable: Solo queue, no click after Commence\n- Killfeed as the game\'s own toasts\n- Sites of Grace switch off during a PvP fight\n- Rivals are hidden from the map, compass and name tags\n\nEveryone in a match needs this version.',
      zip: { name: 'NightreignPvP.zip', sha256: '', size: 31457280 }, game: { version: '1.03.3', exe_sha256: 'X' }, seamless_coop: { version: '1.1.3', nrsc_sha256: '243EEC929A97B71E1E2E3B4215778B89C37D629436B8DD5403E830593D3CE24E' } });
    case 'game_exe_sha256': return wait(200, 'X');
    case 'sc_github': return wait(200, null);
    case 'setup_player': mockPlayer = { steam_id: '76561198000000000', present: true, characters: [{slot: 0, name: 'Champion Of Zeus'}], problem: null }; return wait(100, null);
    case 'replace_starter_save': return wait(100, null);
    case 'import_steam_save': return wait(100, true);
    case 'rename_player': mockPlayer.characters[0].name = args.name; return wait(100, null);
    case 'set_private_code': {
      const c = String(args.code).trim().toLowerCase();
      if (c.length > 32) return Promise.reject('The code can be 32 characters at most.');
      if (!/^[a-z0-9_-]*$/.test(c)) return Promise.reject('The code may only hold letters, digits, - and _.');
      mockRoom = c;
      return wait(150, c);
    }
    default: return wait(400, null);
  }
}
