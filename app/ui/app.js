const T = window.__TAURI__;
const invoke = T.core.invoke;
const listen = T.event.listen;

const AUDIO_EXT = ["mp3", "mp2", "m4a", "mp4", "aac", "alac", "flac", "ogg", "oga", "wav", "aif", "aiff", "aifc", "caf", "mka", "mkv", "webm"];

const $ = (sel, root = document) => root.querySelector(sel);
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
  $(".duration", el).textContent = fmt(d.duration);
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
  if (t.kind === "deck" || t.kind === "playlist") t.el.classList.add("drop-target");
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
      if (at && at.kind === "deck") send({ type: "load_paths", deck: at.deck, paths });
      else send({ type: "add_paths", paths, index: at && at.kind === "row" ? at.index : null });
    } else {
      clearTargets();
    }
  });
}

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
  await loadThemes();
  await setupOsDrop();
  renderPlaylist(0);
  send({ type: "refresh" });
})();
