const T = window.__TAURI__;
const invoke = T.core.invoke;
const listen = T.event.listen;

const AUDIO_EXT = ["mp3", "mp2", "m4a", "mp4", "aac", "alac", "flac", "ogg", "oga", "wav", "aif", "aiff", "aifc", "caf", "mka", "mkv", "webm"];

const $ = (sel, root = document) => root.querySelector(sel);
const ico = (name) => `<svg class="ico"><use href="#i-${name}"/></svg>`;
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

const decks = { A: $('.deck[data-deck="A"]'), B: $('.deck[data-deck="B"]') };
const STATE_LABEL = {
  empty: "VIDE",
  loading: "CHARGEMENT",
  loaded: "PRÊTE",
  playing: "LECTURE",
  paused: "PAUSE",
  finished: "TERMINÉ",
};

let state = null;
let entries = [];
let themes = [];
let rainbow = false;
let crossfaderHeld = false;
const meterHold = { l: 0, r: 0, clipUntil: 0 };

function send(cmd) {
  return invoke("blast", { cmd }).catch((e) => toast("error", String(e)));
}

function fmt(secs) {
  if (!isFinite(secs) || secs < 0) secs = 0;
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  const mm = String(m).padStart(2, "0");
  const ss = String(r).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

function toast(level, message) {
  const el = document.createElement("div");
  el.className = `toast ${level}`;
  el.textContent = message;
  $("#toasts").appendChild(el);
  setTimeout(() => el.remove(), level === "error" ? 8000 : 5000);
}

function dbWidth(peak) {
  if (peak <= 0.00001) return 0;
  const db = 20 * Math.log10(peak);
  return Math.max(0, Math.min(1, (db + 48) / 48));
}

function renderDeck(id, d) {
  const el = decks[id];
  el.classList.toggle("playing", d.state === "playing");
  el.classList.toggle("on-air", state.on_air === id);
  el.classList.toggle("rainbow", rainbow && state.on_air === id);
  $(".chip", el).textContent = STATE_LABEL[d.state] || d.state;
  $(".deck-title", el).textContent = d.title || "—";
  $(".deck-title", el).title = d.path || "";
  $(".deck-artist", el).innerHTML = d.artist ? "" : "&nbsp;";
  if (d.artist) $(".deck-artist", el).textContent = d.artist;
  const remaining = Math.max(0, d.duration - d.position);
  $(".elapsed", el).textContent = fmt(d.position);
  $(".duration", el).textContent = fmt(Math.round(d.duration));
  const rem = $(".remaining", el);
  rem.textContent = "-" + fmt(Math.round(remaining));
  rem.classList.toggle("warn", d.state === "playing" && remaining <= 10);
  const ratio = d.duration > 0 ? d.position / d.duration : 0;
  $(".fill", el).style.width = `${(ratio * 100).toFixed(2)}%`;
  const zone = state.automix && d.duration > 0 ? Math.min(1, state.fade / d.duration) : 0;
  $(".fade-zone", el).style.width = `${(zone * 100).toFixed(2)}%`;
  const has = d.state !== "empty" && d.state !== "loading";
  $('[data-action="play"]', el).disabled = !has || d.state === "playing";
  $('[data-action="pause"]', el).disabled = d.state !== "playing";
  $('[data-action="stop"]', el).disabled = !has;
  $('[data-action="eject"]', el).disabled = !has || d.state === "playing";
  $('[data-action="next"]', el).disabled = d.state === "loading";
}

function renderState() {
  if (!state) return;
  renderDeck("A", state.decks[0]);
  renderDeck("B", state.decks[1]);
  $("#onair").classList.toggle("live", !!state.on_air);
  $("#onair").textContent = state.on_air ? `ON AIR · ${state.on_air}` : "ON AIR";
  if (!crossfaderHeld) $("#crossfader").value = Math.round(state.crossfader * 1000);
  const target = state.crossfader < 0.5 ? "B" : "A";
  $("#fade-btn").innerHTML = `FONDU &rarr; ${target}`;
  $("#fade-btn").dataset.target = target;
  $("#automix").classList.toggle("on", state.automix);
  $("#automix").textContent = state.automix ? "AUTO-MIX ACTIF" : "AUTO-MIX";
  if (document.activeElement !== $("#fade-seconds")) $("#fade-seconds").value = state.fade;
  const now = performance.now();
  meterHold.l = Math.max(dbWidth(state.peak[0]), meterHold.l * 0.85);
  meterHold.r = Math.max(dbWidth(state.peak[1]), meterHold.r * 0.85);
  $("#meter-l").style.width = `${(meterHold.l * 100).toFixed(1)}%`;
  $("#meter-r").style.width = `${(meterHold.r * 100).toFixed(1)}%`;
  if (Math.max(state.peak[0], state.peak[1]) >= 0.999) meterHold.clipUntil = now + 1500;
  $("#clip").classList.toggle("on", now < meterHold.clipUntil);
  $("#device").textContent = state.audio_error
    ? "Aucune sortie audio"
    : `Sortie : ${state.device} · ${state.sample_rate} Hz`;
  $("#audio-error").textContent = state.audio_error ? `Audio indisponible : ${state.audio_error}` : "";
  renderVoice();
  renderPadStates();
}

function renderPlaylist(total) {
  const rows = $("#rows");
  rows.innerHTML = "";
  entries.forEach((e, i) => {
    const row = document.createElement("div");
    row.className = "row row-grid" + (e.error ? " error" : "");
    row.dataset.id = e.id;
    row.dataset.index = i;
    row.title = e.error ? `${e.path}\n${e.error}` : e.path;
    row.innerHTML = `
      <span class="num">${i + 1}</span>
      <span class="title"></span>
      <span class="artist muted"></span>
      <span class="dur">${e.duration != null ? fmt(e.duration) : "--:--"}</span>
      <span class="actions">
        <button class="btn ghost a" data-load="A" title="Charger dans la platine A">A</button>
        <button class="btn ghost b" data-load="B" title="Charger dans la platine B">B</button>
        <button class="btn ghost" data-remove title="Retirer de la playlist">&#10005;</button>
      </span>`;
    $(".title", row).textContent = e.title;
    $(".artist", row).textContent = e.artist;
    rows.appendChild(row);
  });
  const n = entries.length;
  $("#playlist-info").textContent = n === 0 ? "0 titre" : `${n} titre${n > 1 ? "s" : ""} · ${fmt(total)}`;
  $("#empty").classList.toggle("hidden", n > 0);
}

function freeDeck() {
  if (!state) return "A";
  const free = (d) => d.state !== "playing" && d.state !== "loading";
  const order = state.on_air === "A" ? ["B", "A"] : ["A", "B"];
  for (const id of order) {
    const d = state.decks[id === "A" ? 0 : 1];
    if (free(d) && (d.state === "empty" || d.state === "finished")) return id;
  }
  for (const id of order) {
    if (free(state.decks[id === "A" ? 0 : 1])) return id;
  }
  return null;
}

function applyTheme(id) {
  const theme = themes.find((t) => t.id === id) || themes[0];
  if (!theme) return;
  const root = document.documentElement;
  for (const section of ["colors", "meters"]) {
    for (const [k, v] of Object.entries(theme[section] || {})) {
      root.style.setProperty(`--${k.replaceAll("_", "-")}`, v);
    }
  }
  const fx = theme.effects || {};
  root.style.setProperty("--rainbow-speed", `${3 / Math.max(0.1, fx.rainbow_speed || 1)}s`);
  $("#theme-select").value = theme.id;
  try { localStorage.setItem("blast.theme", theme.id); } catch (_) {}
  if (localStorageGet("blast.rainbow") === null) setRainbow(!!fx.deck_rainbow, false);
}

function localStorageGet(k) {
  try { return localStorage.getItem(k); } catch (_) { return null; }
}

function setRainbow(on, persist = true) {
  rainbow = on;
  $("#rainbow-toggle").classList.toggle("on", on);
  if (persist) {
    try { localStorage.setItem("blast.rainbow", on ? "1" : "0"); } catch (_) {}
  }
  renderState();
}

async function loadThemes() {
  try {
    themes = await invoke("themes");
  } catch (e) {
    themes = [];
  }
  const sel = $("#theme-select");
  sel.innerHTML = "";
  for (const t of themes) {
    const o = document.createElement("option");
    o.value = t.id;
    o.textContent = (t.meta && t.meta.name) || t.id;
    sel.appendChild(o);
  }
  applyTheme(localStorageGet("blast.theme") || "black-lab");
  const r = localStorageGet("blast.rainbow");
  if (r !== null) setRainbow(r === "1", false);
}

function targetAt(x, y) {
  const el = document.elementFromPoint(x, y);
  if (!el) return null;
  const pad = el.closest(".pad");
  if (pad) return { kind: "pad", index: Number(pad.dataset.index), el: pad };
  const deck = el.closest(".deck");
  if (deck) return { kind: "deck", deck: deck.dataset.deck, el: deck };
  const row = el.closest(".row");
  if (row) {
    const r = row.getBoundingClientRect();
    const after = y > r.top + r.height / 2;
    return { kind: "row", index: Number(row.dataset.index) + (after ? 1 : 0), el: row, after };
  }
  if (el.closest(".playlist")) return { kind: "playlist", index: entries.length, el: $("#playlist") };
  return null;
}

function clearTargets() {
  $$(".drop-target").forEach((e) => e.classList.remove("drop-target"));
  $$(".drop-before, .drop-after").forEach((e) => e.classList.remove("drop-before", "drop-after"));
}

function showTarget(t) {
  clearTargets();
  if (!t) return;
  if (t.kind === "deck" || t.kind === "playlist" || t.kind === "pad") t.el.classList.add("drop-target");
  if (t.kind === "row") t.el.classList.add(t.after ? "drop-after" : "drop-before");
}

let drag = null;

$("#rows").addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || e.target.closest("button")) return;
  const row = e.target.closest(".row");
  if (!row) return;
  drag = { id: Number(row.dataset.id), row, x: e.clientX, y: e.clientY, active: false };
});

document.addEventListener("pointermove", (e) => {
  if (!drag) return;
  if (!drag.active) {
    if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 6) return;
    drag.active = true;
    drag.row.classList.add("dragging-src");
    document.body.classList.add("dragging");
    const ghost = $("#ghost");
    ghost.textContent = $(".title", drag.row).textContent;
    ghost.style.display = "block";
  }
  const ghost = $("#ghost");
  ghost.style.left = `${e.clientX + 14}px`;
  ghost.style.top = `${e.clientY + 10}px`;
  showTarget(targetAt(e.clientX, e.clientY));
});

document.addEventListener("pointerup", (e) => {
  if (!drag) return;
  if (drag.active) {
    const t = targetAt(e.clientX, e.clientY);
    if (t && t.kind === "deck") send({ type: "load_entry", id: drag.id, deck: t.deck });
    else if (t && t.kind === "pad") {
      const e = entries.find((x) => x.id === drag.id);
      if (e) assignPad(t.index, e.path, e.title);
    }
    else if (t && (t.kind === "row" || t.kind === "playlist")) send({ type: "move", id: drag.id, to: t.index });
  }
  drag.row.classList.remove("dragging-src");
  document.body.classList.remove("dragging");
  $("#ghost").style.display = "none";
  clearTargets();
  drag = null;
});

$("#rows").addEventListener("dblclick", (e) => {
  const row = e.target.closest(".row");
  if (!row || e.target.closest("button")) return;
  const deck = freeDeck();
  if (deck) send({ type: "load_entry", id: Number(row.dataset.id), deck });
  else toast("warn", "Aucune platine libre.");
});

$("#rows").addEventListener("click", (e) => {
  const btn = e.target.closest("button");
  if (!btn) return;
  const id = Number(btn.closest(".row").dataset.id);
  if (btn.dataset.load) send({ type: "load_entry", id, deck: btn.dataset.load });
  else if ("remove" in btn.dataset) send({ type: "remove", id });
});

for (const [id, el] of Object.entries(decks)) {
  el.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-action]");
    if (!btn) return;
    send({ type: btn.dataset.action, deck: id });
  });
  $(".progress", el).addEventListener("click", (e) => {
    const r = e.currentTarget.getBoundingClientRect();
    send({ type: "seek", deck: id, fraction: (e.clientX - r.left) / r.width });
  });
}

const xf = $("#crossfader");
xf.addEventListener("pointerdown", () => (crossfaderHeld = true));
document.addEventListener("pointerup", () => (crossfaderHeld = false));
let xfPending = false;
xf.addEventListener("input", () => {
  if (xfPending) return;
  xfPending = true;
  requestAnimationFrame(() => {
    xfPending = false;
    send({ type: "crossfader", value: Number(xf.value) / 1000 });
  });
});

$("#fade-btn").addEventListener("click", (e) => send({ type: "crossfade", to: e.currentTarget.dataset.target || "B" }));
$("#automix").addEventListener("click", () => send({ type: "auto_mix", enabled: !(state && state.automix) }));
$("#fade-seconds").addEventListener("change", (e) => send({ type: "fade_seconds", value: Number(e.target.value) || 0 }));
$("#clear").addEventListener("click", () => {
  if (entries.length) send({ type: "clear" });
});

$("#add-files").addEventListener("click", async () => {
  const picked = await T.dialog.open({
    multiple: true,
    directory: false,
    title: "Ajouter des titres",
    filters: [{ name: "Audio", extensions: AUDIO_EXT }],
  });
  const paths = picked == null ? [] : [].concat(picked);
  if (paths.length) send({ type: "add_paths", paths, index: null });
});

$("#add-folder").addEventListener("click", async () => {
  const picked = await T.dialog.open({ multiple: true, directory: true, title: "Ajouter un dossier" });
  const paths = picked == null ? [] : [].concat(picked);
  if (paths.length) send({ type: "add_paths", paths, index: null });
});

$("#theme-select").addEventListener("change", (e) => applyTheme(e.target.value));
$("#rainbow-toggle").addEventListener("click", () => setRainbow(!rainbow));
$("#themes-folder").addEventListener("click", async () => {
  try {
    const dir = await invoke("themes_dir");
    toast("info", `Dépose tes thèmes .toml dans : ${dir} — puis relance BLAST.`);
  } catch (e) {
    toast("error", String(e));
  }
});

function tickClock() {
  const d = new Date();
  $("#clock").textContent = d.toLocaleTimeString("fr-FR", { hour12: false });
}
setInterval(tickClock, 500);
tickClock();

async function setupOsDrop() {
  const wv = T.webview && T.webview.getCurrentWebview ? T.webview.getCurrentWebview() : null;
  if (!wv || !wv.onDragDropEvent) return;
  await wv.onDragDropEvent((event) => {
    const p = event.payload;
    const dpr = window.devicePixelRatio || 1;
    const at = p.position ? targetAt(p.position.x / dpr, p.position.y / dpr) : null;
    if (p.type === "enter" || p.type === "over") {
      showTarget(at || { kind: "playlist", index: entries.length, el: $("#playlist") });
    } else if (p.type === "drop") {
      clearTargets();
      const paths = p.paths || [];
      if (!paths.length) return;
      if (at && at.kind === "pad") assignPad(at.index, paths[0], "");
      else if (at && at.kind === "deck") send({ type: "load_paths", deck: at.deck, paths });
      else send({ type: "add_paths", paths, index: at && at.kind === "row" ? at.index : null });
    } else {
      clearTargets();
    }
  });
}


const BITRATES = [32, 40, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const STATUS_LABEL = { off: "Arrêtée", connecting: "Connexion…", live: "En direct", retrying: "Reconnexion", error: "Erreur" };
let cast = { casting: false, song: "", outputs: [] };
let settings = null;
let selectedOut = null;

function statusOf(o) {
  return (o && o.status && o.status.state) || "off";
}

function renderCast() {
  const btn = $("#cast-btn");
  const active = cast.outputs.filter((o) => o.enabled);
  const states = active.map(statusOf);
  const live = states.filter((s) => s === "live").length;
  btn.className = "cast-btn";
  let label = "DIFFUSER";
  if (cast.casting) {
    if (states.includes("error")) btn.classList.add("error");
    else if (live === active.length && live > 0) btn.classList.add("live");
    else if (states.includes("retrying")) btn.classList.add("retrying");
    else btn.classList.add("connecting");
    label = live > 0
      ? `EN DIRECT ${live}/${active.length}`
      : states.includes("error")
        ? "ERREUR"
        : states.includes("retrying")
          ? "RECONNEXION…"
          : "CONNEXION…";
  }
  $("#cast-label").textContent = label;
  renderCastTimer();
  const lis = $("#cast-listeners");
  const hasCount = cast.casting && cast.listeners != null;
  lis.classList.toggle("hidden", !hasCount);
  if (hasCount) {
    lis.innerHTML = `${ico("user")} <b>${cast.listeners}</b> · max ${Math.max(cast.session_peak || 0, ...cast.outputs.map((o) => o.peak || 0))}`;
    lis.title = "Auditeurs en cours · pic d'auditeurs (serveur et session)";
  }
  btn.title = cast.outputs.length
    ? cast.outputs.map((o) => `${o.name} (${o.target}) : ${STATUS_LABEL[statusOf(o)]}${o.listeners != null ? " · " + o.listeners + " auditeur(s), max " + (o.peak || o.listeners) : ""}${o.status && o.status.detail ? " — " + o.status.detail : ""}`).join("\n")
    : "Aucune sortie configurée : ouvre les Réglages";
  const errs = cast.outputs.filter((o) => o.status && o.status.detail);
  $("#cast-summary").textContent = cast.casting
    ? `Diffusion : ${live}/${active.length} en direct${cast.song ? " · " + cast.song : ""}${errs.length ? " · " + errs[0].name + " : " + errs[0].status.detail : ""}`
    : "";
  if (settings) renderOutList();
}

function renderCastTimer() {
  const t = $("#cast-time");
  const on = cast.casting && cast.since;
  t.classList.toggle("hidden", !on);
  if (on) {
    const s = Math.max(0, Math.floor((Date.now() - cast.since) / 1000));
    t.textContent = [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60].map((n) => String(n).padStart(2, "0")).join(":");
    t.title = "Durée de diffusion";
  }
}
setInterval(renderCastTimer, 1000);

function castStatusFor(id) {
  const o = cast.outputs.find((x) => x.id === id);
  return o ? o.status : null;
}

function renderOutList() {
  const box = $("#out-items");
  box.innerHTML = "";
  for (const o of settings.outputs) {
    const st = castStatusFor(o.id);
    const state = (st && st.state) || "off";
    const el = document.createElement("div");
    el.className = "out-item" + (o.id === selectedOut ? " active" : "") + (o.enabled ? "" : " disabled");
    el.innerHTML = `<div class="n"><span class="dot ${state}"></span><span></span></div><div class="t"></div><div class="s muted"></div>`;
    $(".n span:last-child", el).textContent = o.name;
    $(".t", el).textContent = `${o.kind === "icecast" ? "Icecast" : o.kind === "shoutcast_v1" ? "Shoutcast v1" : "Shoutcast v2"} · ${o.host}:${o.port}${o.kind === "icecast" ? o.mount : ""} · ${o.bitrate} kb/s`;
    const co = cast.outputs.find((x) => x.id === o.id);
    const count = co && co.listeners != null ? ` · ${co.listeners} auditeur${co.listeners > 1 ? "s" : ""} (max ${co.peak || co.listeners})` : "";
    $(".s", el).textContent = (o.enabled ? STATUS_LABEL[state] : "Désactivée") + count + (st && st.detail ? " — " + st.detail : "") + (o.has_password ? "" : " · mot de passe à saisir");
    el.addEventListener("click", () => selectOutput(o.id));
    box.appendChild(el);
  }
  const has = settings.outputs.length > 0 || selectedOut === "new";
  $("#out-empty").classList.toggle("hidden", has);
  $("#out-body").classList.toggle("hidden", !has);
}

function formValues(form) {
  const v = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    if (el.type === "checkbox") v[el.name] = el.checked;
    else if (el.type === "number") v[el.name] = Number(el.value);
    else v[el.name] = el.value;
  }
  return v;
}

function fillForm(form, data) {
  for (const el of form.elements) {
    if (!el.name || !(el.name in data)) continue;
    if (el.type === "checkbox") el.checked = !!data[el.name];
    else el.value = String(data[el.name]);
  }
}

function setResult(id, kind, text) {
  const el = $(id);
  el.className = `form-result ${kind}`;
  el.textContent = text;
}

function applyKindFields() {
  const kind = $("#out-form").elements.kind.value;
  $$("#out-form [data-for]").forEach((el) => el.classList.toggle("hidden", el.dataset.for !== kind));
}

function selectOutput(id) {
  selectedOut = id;
  const form = $("#out-form");
  const o = settings.outputs.find((x) => x.id === id) || {
    id: "", name: "Ma webradio", enabled: true, kind: "icecast", host: "localhost", port: 8000,
    mount: "/live", sid: 1, user: "source", bitrate: 128, mono: false, station: "BLAST Broadcaster",
    genre: "", description: "", url: "", public: false, has_password: false,
  };
  fillForm(form, { ...o, mono: String(o.mono) });
  form.elements.password.value = "";
  form.elements.password.placeholder = o.has_password ? "•••••• enregistré (laisser vide pour garder)" : "mot de passe de la source";
  $("#out-delete").disabled = !o.id;
  setResult("#out-result", "", "");
  applyKindFields();
  renderOutList();
}

function currentOutput() {
  const v = formValues($("#out-form"));
  const id = selectedOut && selectedOut !== "new" ? selectedOut : "";
  const out = {
    id, name: v.name, enabled: v.enabled, kind: v.kind, host: v.host.trim(), port: v.port,
    mount: v.mount, sid: v.sid || 1, user: v.user, bitrate: Number(v.bitrate), mono: v.mono === "true",
    station: v.station, genre: v.genre, description: v.description, url: v.url, public: v.public,
  };
  return { out, password: v.password || null };
}

function applySettings(view, keepSelection = true) {
  settings = view;
  $("#store-info").textContent = `Mots de passe : ${view.secret_store}`;
  if (view.warning) toast("warn", view.warning);
  if (!keepSelection || !view.outputs.some((o) => o.id === selectedOut)) {
    selectedOut = view.outputs.length ? view.outputs[0].id : null;
  }
  if (selectedOut) selectOutput(selectedOut);
  else renderOutList();
  fillForm($("#aura-form"), view.aura);
  $("#aura-form").elements.password.value = "";
  $("#aura-form").elements.password.placeholder = view.aura_has_password ? "•••••• enregistré" : "vide si OBS n'en demande pas";
  const dev = $("#audio-form").elements.device;
  dev.innerHTML = "";
  const def = document.createElement("option");
  def.value = "";
  def.textContent = `Sortie par défaut du système${view.default_device ? " (" + view.default_device + ")" : ""}`;
  dev.appendChild(def);
  for (const d of view.devices) {
    const o = document.createElement("option");
    o.value = d;
    o.textContent = d;
    dev.appendChild(o);
  }
  dev.value = view.audio.device || "";
  const mic = $("#mic-form").elements.device;
  mic.innerHTML = "";
  const mdef = document.createElement("option");
  mdef.value = "";
  mdef.textContent = `Micro par défaut du système${view.default_input ? " (" + view.default_input + ")" : ""}`;
  mic.appendChild(mdef);
  for (const d of view.input_devices) {
    const o = document.createElement("option");
    o.value = d;
    o.textContent = d;
    mic.appendChild(o);
  }
  mic.value = view.voice.input_device || "";
  $("#mic-form").elements.enabled.checked = !!view.voice.enabled;
  $("#mic-status").textContent = state && state.mic
    ? state.mic.available
      ? `Micro actif : ${state.mic.name}`
      : state.mic.error
        ? `Micro indisponible : ${state.mic.error}`
        : "Aucun micro détecté."
    : "";
}

$("#mic-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const f = $("#mic-form").elements;
  try {
    applySettings(await invoke("save_input", { enabled: f.enabled.checked, device: f.device.value || null }));
    setResult("#mic-result", "ok", "Enregistré.");
  } catch (err) {
    setResult("#mic-result", "err", String(err));
  }
});

async function openSettings(tab) {
  try {
    applySettings(await invoke("get_settings"), false);
  } catch (e) {
    toast("error", String(e));
    return;
  }
  if (tab) showTab(tab);
  $("#settings").classList.remove("hidden");
}

function showTab(tab) {
  $$(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === tab));
  $$(".tab-page").forEach((p) => p.classList.toggle("hidden", p.dataset.page !== tab));
}

(() => {
  const sel = $("#out-form").elements.bitrate;
  for (const b of BITRATES) {
    const o = document.createElement("option");
    o.value = b;
    o.textContent = `${b} kb/s`;
    sel.appendChild(o);
  }
})();

$("#settings-btn").addEventListener("click", () => openSettings());
$("#settings-close").addEventListener("click", () => $("#settings").classList.add("hidden"));
$("#settings").addEventListener("pointerdown", (e) => {
  if (e.target.id === "settings") $("#settings").classList.add("hidden");
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") $("#settings").classList.add("hidden");
});
$$(".tab").forEach((t) => t.addEventListener("click", () => showTab(t.dataset.tab)));
$("#out-form").elements.kind.addEventListener("change", applyKindFields);
$("#out-add").addEventListener("click", () => selectOutput("new"));

$("#out-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const { out, password } = currentOutput();
  try {
    const view = await invoke("save_output", { output: out, password });
    selectedOut = out.id || (view.outputs.length ? view.outputs[view.outputs.length - 1].id : null);
    applySettings(view);
    setResult("#out-result", "ok", "Enregistré.");
  } catch (err) {
    setResult("#out-result", "err", String(err));
  }
});

$("#out-test").addEventListener("click", async () => {
  const { out, password } = currentOutput();
  setResult("#out-result", "busy", "Connexion en cours…");
  try {
    setResult("#out-result", "ok", await invoke("test_output", { output: out, password }));
  } catch (err) {
    setResult("#out-result", "err", String(err));
  }
});

$("#out-delete").addEventListener("click", async () => {
  if (!selectedOut || selectedOut === "new") return;
  const o = settings.outputs.find((x) => x.id === selectedOut);
  if (!o) return;
  if ($("#out-delete").dataset.confirm !== selectedOut) {
    $("#out-delete").dataset.confirm = selectedOut;
    $("#out-delete").textContent = "Confirmer la suppression";
    setTimeout(() => {
      $("#out-delete").dataset.confirm = "";
      $("#out-delete").textContent = "Supprimer";
    }, 4000);
    return;
  }
  $("#out-delete").dataset.confirm = "";
  $("#out-delete").textContent = "Supprimer";
  try {
    applySettings(await invoke("delete_output", { id: o.id }), false);
  } catch (err) {
    setResult("#out-result", "err", String(err));
  }
});

function auraValues() {
  const v = formValues($("#aura-form"));
  return {
    aura: { host: v.host.trim(), port: v.port, auto_connect: v.auto_connect, now_playing_source: v.now_playing_source },
    password: v.password || null,
  };
}

$("#aura-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    applySettings(await invoke("save_aura", auraValues()));
    setResult("#aura-result", "ok", "Enregistré.");
  } catch (err) {
    setResult("#aura-result", "err", String(err));
  }
});

$("#aura-test").addEventListener("click", async () => {
  setResult("#aura-result", "busy", "Connexion à OBS…");
  try {
    setResult("#aura-result", "ok", await invoke("test_aura", auraValues()));
  } catch (err) {
    setResult("#aura-result", "err", String(err));
  }
});

$("#audio-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    applySettings(await invoke("save_audio", { device: $("#audio-form").elements.device.value || null }));
    setResult("#audio-result", "ok", "Enregistré.");
  } catch (err) {
    setResult("#audio-result", "err", String(err));
  }
});

$("#cast-btn").addEventListener("click", () => {
  if (!cast.outputs.length) {
    openSettings("cast");
    return;
  }
  send({ type: cast.casting ? "cast_stop" : "cast_start" });
});


const PADS_PER_PAGE = 12;
let padsData = { pads: [], count: 36 };
let padPage = 0;
let voice = null;
let micLatched = false;
let pttHeld = false;
let menuPad = null;

function setMic(open) {
  send({ type: "mic_open", open });
}

function renderVoice() {
  if (!state || !state.mic) return;
  const m = state.mic;
  $("#voicefx").classList.toggle("disabled", !m.available);
  $("#mic-name").textContent = m.available ? m.name || "" : m.error ? "Micro indisponible" : "Aucun micro";
  $("#mic-name").title = m.error || m.name || "";
  $("#mic-toggle").classList.toggle("on", micLatched && m.open);
  $("#mic-toggle").innerHTML = micLatched && m.open ? `${ico("unlock")}<span>OUVERT</span>` : `${ico("lock")}<span>VERROU</span>`;
  $("#ptt").classList.toggle("active", m.open);
  $("#meter-mic").style.width = `${(dbWidth(m.peak) * 100).toFixed(1)}%`;
}

function padColor(i) {
  const palette = ["var(--accent)", "var(--deck-b)", "var(--meter-mid)", "var(--meter-low)"];
  return palette[Math.floor(i / 4) % palette.length];
}

function renderPads() {
  const pages = Math.max(1, Math.ceil(padsData.count / PADS_PER_PAGE));
  padPage = Math.min(padPage, pages - 1);
  $("#pad-page").textContent = `${padPage + 1}/${pages}`;
  const box = $("#pads");
  box.innerHTML = "";
  for (let k = 0; k < PADS_PER_PAGE; k++) {
    const index = padPage * PADS_PER_PAGE + k;
    if (index >= padsData.count) break;
    const p = padsData.pads.find((x) => x.index === index);
    const el = document.createElement("button");
    el.className = "pad" + (p ? "" : " vacant") + (p && !p.ready && !p.error ? " loading" : "") + (p && p.error ? " error" : "");
    el.dataset.index = index;
    el.style.setProperty("--pad-color", (p && p.color) || padColor(k));
    el.title = p ? `${p.name}\n${p.path}${p.error ? "\n⚠ " + p.error : ""}\nClic droit : modifier` : "Pad vide : clic pour choisir un son, ou glisse un fichier ici";
    const label = document.createElement("span");
    label.textContent = p ? p.name : "+";
    el.appendChild(label);
    const key = document.createElement("span");
    key.className = "key";
    key.textContent = `F${k + 1}`;
    el.appendChild(key);
    box.appendChild(el);
  }
  renderPadStates();
}

function renderPadStates() {
  const mask = state ? BigInt(state.sfx || 0) : 0n;
  $$("#pads .pad").forEach((el) => {
    const i = BigInt(el.dataset.index);
    el.classList.toggle("playing", ((mask >> i) & 1n) === 1n);
  });
}

async function pickSound() {
  const picked = await T.dialog.open({ multiple: false, directory: false, title: "Choisir un son", filters: [{ name: "Audio", extensions: AUDIO_EXT }] });
  return picked == null ? null : [].concat(picked)[0];
}

async function assignPad(index, path, name) {
  try {
    await invoke("save_pad", { pad: { index, name: name || "", path, color: "" } });
  } catch (e) {
    toast("error", String(e));
  }
}

function playPad(index) {
  const p = padsData.pads.find((x) => x.index === index);
  if (p) send({ type: "pad_play", index });
  return !!p;
}

function closePadMenu() {
  $("#pad-menu").classList.add("hidden");
  menuPad = null;
}

$("#pads").addEventListener("click", async (e) => {
  const el = e.target.closest(".pad");
  if (!el) return;
  const index = Number(el.dataset.index);
  if (!playPad(index)) {
    const path = await pickSound();
    if (path) assignPad(index, path, "");
  }
});

$("#pads").addEventListener("contextmenu", (e) => {
  const el = e.target.closest(".pad");
  if (!el) return;
  e.preventDefault();
  e.stopPropagation();
  menuPad = Number(el.dataset.index);
  const p = padsData.pads.find((x) => x.index === menuPad);
  $("#pad-name").value = p ? p.name : "";
  $("#pad-rename").disabled = !p;
  $("#pad-clear").disabled = !p;
  const menu = $("#pad-menu");
  menu.classList.remove("hidden");
  const r = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, window.innerWidth - r.width - 8)}px`;
  menu.style.top = `${Math.min(e.clientY, window.innerHeight - r.height - 8)}px`;
  $("#pad-name").focus();
});

document.addEventListener("pointerdown", (e) => {
  if (menuPad !== null && !e.target.closest("#pad-menu")) closePadMenu();
});

$("#pad-pick").addEventListener("click", async () => {
  const index = menuPad;
  closePadMenu();
  const path = await pickSound();
  if (path != null) assignPad(index, path, "");
});

async function renameFromMenu() {
  const p = padsData.pads.find((x) => x.index === menuPad);
  const name = $("#pad-name").value.trim();
  closePadMenu();
  if (p && name) assignPad(p.index, p.path, name);
}
$("#pad-rename").addEventListener("click", renameFromMenu);
$("#pad-name").addEventListener("keydown", (e) => {
  if (e.key === "Enter") renameFromMenu();
});
$("#pad-clear").addEventListener("click", async () => {
  const index = menuPad;
  closePadMenu();
  try {
    await invoke("clear_pad", { index });
  } catch (err) {
    toast("error", String(err));
  }
});

$("#pad-prev").addEventListener("click", () => {
  padPage = Math.max(0, padPage - 1);
  renderPads();
});
$("#pad-next").addEventListener("click", () => {
  const pages = Math.ceil(padsData.count / PADS_PER_PAGE);
  padPage = Math.min(pages - 1, padPage + 1);
  renderPads();
});
$("#sfx-stop").addEventListener("click", () => send({ type: "pad_stop_all" }));
$("#sfx-vol").addEventListener("change", (e) => {
  invoke("save_sfx_volume", { volumeDb: Number(e.target.value) }).catch((err) => toast("error", String(err)));
});

$("#mic-toggle").addEventListener("click", () => {
  micLatched = !(micLatched && state && state.mic.open);
  setMic(micLatched);
});

function pttDown() {
  if (pttHeld) return;
  pttHeld = true;
  setMic(true);
}
function pttUp() {
  if (!pttHeld) return;
  pttHeld = false;
  if (!micLatched) setMic(false);
}
$("#ptt").addEventListener("pointerdown", (e) => {
  e.preventDefault();
  $("#ptt").setPointerCapture(e.pointerId);
  pttDown();
});
$("#ptt").addEventListener("pointerup", pttUp);
$("#ptt").addEventListener("pointercancel", pttUp);
window.addEventListener("blur", pttUp);

const VOICE_SLIDERS = [
  ["#mic-gain", "mic_gain_db", (v) => `${v > 0 ? "+" : ""}${v} dB`],
  ["#music-gain", "music_gain_db", (v) => `${v > 0 ? "+" : ""}${v} dB`],
  ["#duck-db", "duck_db", (v) => `${v} dB`],
  ["#eq-low", "low_db", (v) => `${v > 0 ? "+" : ""}${v} dB`],
  ["#eq-mid", "mid_db", (v) => `${v > 0 ? "+" : ""}${v} dB`],
  ["#eq-high", "high_db", (v) => `${v > 0 ? "+" : ""}${v} dB`],
];
let voiceTimer = null;

function saveVoiceSoon() {
  clearTimeout(voiceTimer);
  voiceTimer = setTimeout(() => {
    invoke("save_voice", { voice }).catch((e) => toast("error", String(e)));
  }, 250);
}

function applyVoice(v) {
  voice = { ...v };
  for (const [sel, key, fmtv] of VOICE_SLIDERS) {
    $(sel).value = voice[key];
    $(sel + "-v").textContent = fmtv(voice[key]);
  }
  $("#eq-hpf").checked = !!voice.highpass;
  $("#mic-monitor").checked = !!voice.monitor;
  $("#duck-toggle").classList.toggle("on", !!voice.duck);
}

for (const [sel, key, fmtv] of VOICE_SLIDERS) {
  $(sel).addEventListener("input", (e) => {
    if (!voice) return;
    voice[key] = Number(e.target.value);
    $(sel + "-v").textContent = fmtv(voice[key]);
    saveVoiceSoon();
  });
}
$("#eq-hpf").addEventListener("change", (e) => {
  if (!voice) return;
  voice.highpass = e.target.checked;
  saveVoiceSoon();
});
$("#mic-monitor").addEventListener("change", (e) => {
  if (!voice) return;
  voice.monitor = e.target.checked;
  saveVoiceSoon();
});
$("#duck-toggle").addEventListener("click", () => {
  if (!voice) return;
  voice.duck = !voice.duck;
  $("#duck-toggle").classList.toggle("on", voice.duck);
  saveVoiceSoon();
});

function typingTarget(e) {
  const t = e.target;
  return t && (t.tagName === "INPUT" || t.tagName === "SELECT" || t.tagName === "TEXTAREA");
}

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") closePadMenu();
  if (!$("#settings").classList.contains("hidden") || typingTarget(e)) return;
  if (e.code === "Space") {
    e.preventDefault();
    if (!e.repeat) pttDown();
    return;
  }
  if (e.key === "m" || e.key === "M") {
    if (!e.repeat) $("#mic-toggle").click();
    return;
  }
  const f = /^F([1-9]|1[0-2])$/.exec(e.key);
  if (f) {
    e.preventDefault();
    if (!e.repeat) playPad(padPage * PADS_PER_PAGE + Number(f[1]) - 1);
  }
});
document.addEventListener("keyup", (e) => {
  if (e.code === "Space") {
    e.preventDefault();
    pttUp();
  }
});

async function loadVoiceSettings() {
  try {
    const view = await invoke("get_settings");
    applyVoice(view.voice);
    $("#sfx-vol").value = view.soundboard.volume_db;
  } catch (e) {
    toast("error", String(e));
  }
}


let obs = { status: "off", scenes: [], inputs: [] };
let obsInput = null;
let obsVolHeld = false;
const OBS_STATUS = { off: "Déconnecté", connecting: "Connexion…", connected: "Connecté", retrying: "Reconnexion…", error: "Erreur" };

function hms(ms) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60].map((n) => String(n).padStart(2, "0")).join(":");
}

function showFx(page) {
  $$(".fx-tab").forEach((t) => t.classList.toggle("active", t.dataset.fx === page));
  $$("#soundfx [data-fxpage]").forEach((el) => (el.hidden = el.dataset.fxpage !== page));
  try { localStorage.setItem("blast.fxtab", page); } catch (_) {}
}

function renderObsTimers() {
  const now = Date.now();
  const live = $("#obs-live");
  live.classList.toggle("hidden", !(obs.stream && obs.stream_since));
  if (obs.stream && obs.stream_since) $("span", live).textContent = hms(now - obs.stream_since);
  const rec = $("#obs-rec");
  rec.classList.toggle("hidden", !(obs.record && obs.record_since));
  rec.classList.toggle("paused", !!obs.record_paused);
  if (obs.record && obs.record_since && !obs.record_paused) $("span", rec).textContent = hms(now - obs.record_since);
  $("use", rec).setAttribute("href", obs.record_paused ? "#i-pause" : "#i-rec");
}

function renderObs() {
  const st = obs.status;
  const connected = st === "connected";
  $("#obs-pill").className = `obs-pill ${st}`;
  $("#obs-pill").title = `OBS : ${OBS_STATUS[st] || st}${obs.detail ? " — " + obs.detail : ""}`;
  $(".fx-tab[data-fx='obs']").classList.toggle("obs-on", connected);
  $("#obs-dot").className = `dot ${connected ? "live" : st === "error" ? "error" : st === "off" ? "" : "connecting"}`;
  $("#obs-status-text").textContent = connected
    ? `Connecté — OBS ${obs.obs_version || ""}`
    : `${OBS_STATUS[st] || st}${obs.detail ? " : " + obs.detail : ""}`;
  $("#obs-status-text").title = obs.detail || "";
  $("#obs-connect").textContent = st === "off" || st === "error" ? "Connecter" : "Déconnecter";
  $$(".obs-grid .hw").forEach((b) => (b.disabled = !connected));
  $("#obs-pause").disabled = !connected || !obs.record;
  $("#obs-stream").classList.toggle("on-stream", !!obs.stream);
  $("#obs-stream small").textContent = obs.stream ? "STOP STREAM" : "STREAM";
  $("#obs-record").classList.toggle("on-rec", !!obs.record);
  $("#obs-record small").textContent = obs.record ? "STOP REC" : "REC";
  $("#obs-pause").classList.toggle("on-warn", !!obs.record_paused);
  $("#obs-pause small").textContent = obs.record_paused ? "REPRENDRE" : "PAUSE REC";

  const box = $("#obs-scenes");
  box.innerHTML = "";
  if (!connected || !obs.scenes.length) {
    const e = document.createElement("div");
    e.className = "obs-empty";
    e.textContent = connected ? "Aucune scène dans OBS" : "Connecte OBS pour afficher tes scènes";
    box.appendChild(e);
  } else {
    for (const name of obs.scenes) {
      const b = document.createElement("button");
      b.className = "scene" + (name === obs.current ? " current" : "");
      b.textContent = name;
      b.title = name;
      b.addEventListener("click", () => send({ type: "obs", action: "scene", name }));
      box.appendChild(b);
    }
  }

  const sel = $("#obs-input");
  const names = obs.inputs.map((i) => i.name);
  if (!names.includes(obsInput)) obsInput = names[0] || null;
  sel.innerHTML = "";
  if (!names.length) {
    const o = document.createElement("option");
    o.textContent = connected ? "Aucune source audio" : "Source audio OBS";
    o.value = "";
    sel.appendChild(o);
  }
  for (const i of obs.inputs) {
    const o = document.createElement("option");
    o.value = i.name;
    o.textContent = (i.muted ? "[muet] " : "") + i.name;
    sel.appendChild(o);
  }
  sel.value = obsInput || "";
  sel.disabled = !connected || !names.length;
  const cur = obs.inputs.find((i) => i.name === obsInput);
  $("#obs-vol").disabled = !cur;
  if (!obsVolHeld) $("#obs-vol").value = cur ? Math.round(cur.volume * 100) : 0;
  $("#obs-vol-v").textContent = cur ? `${Math.round(cur.volume * 100)}%` : "–";
  $("#obs-mute").disabled = !cur;
  $("#obs-mute").classList.toggle("on-warn", !!(cur && cur.muted));
  renderObsTimers();
}

$$(".fx-tab").forEach((t) => t.addEventListener("click", () => showFx(t.dataset.fx)));
$("#obs-pill").addEventListener("click", () => {
  showFx("obs");
  if (obs.status === "off" || obs.status === "error") send({ type: "obs", action: "connect" });
});
$("#obs-connect").addEventListener("click", () => {
  send({ type: "obs", action: obs.status === "off" || obs.status === "error" ? "connect" : "disconnect" });
});
$$(".obs-grid [data-obs]").forEach((b) => b.addEventListener("click", () => send({ type: "obs", action: b.dataset.obs })));
$("#obs-mute").addEventListener("click", () => {
  if (obsInput) send({ type: "obs", action: "mute", input: obsInput });
});
$("#obs-input").addEventListener("change", (e) => {
  obsInput = e.target.value || null;
  renderObs();
});
$("#obs-vol").addEventListener("pointerdown", () => (obsVolHeld = true));
document.addEventListener("pointerup", () => (obsVolHeld = false));
let obsVolPending = false;
$("#obs-vol").addEventListener("input", (e) => {
  $("#obs-vol-v").textContent = `${e.target.value}%`;
  if (obsVolPending || !obsInput) return;
  obsVolPending = true;
  setTimeout(() => {
    obsVolPending = false;
    send({ type: "obs", action: "volume", input: obsInput, mul: Number($("#obs-vol").value) / 100 });
  }, 60);
});
setInterval(renderObsTimers, 500);

window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  await listen("blast://state", (e) => {
    state = e.payload;
    renderState();
  });
  await listen("blast://playlist", (e) => {
    entries = e.payload.entries;
    renderPlaylist(e.payload.total);
  });
  await listen("blast://notice", (e) => toast(e.payload.level, e.payload.message));
  await listen("blast://obs", (e) => {
    obs = e.payload;
    renderObs();
  });
  await listen("blast://pads", (e) => {
    padsData = e.payload;
    renderPads();
  });
  await listen("blast://cast", (e) => {
    cast = e.payload;
    renderCast();
  });
  await loadThemes();
  await loadVoiceSettings();
  renderPads();
  renderObs();
  showFx(localStorageGet("blast.fxtab") === "obs" ? "obs" : "pads");
  await setupOsDrop();
  renderPlaylist(0);
  send({ type: "refresh" });
})();
