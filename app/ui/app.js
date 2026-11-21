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
  btn.title = cast.outputs.length
    ? cast.outputs.map((o) => `${o.name} (${o.target}) : ${STATUS_LABEL[statusOf(o)]}${o.status && o.status.detail ? " — " + o.status.detail : ""}`).join("\n")
    : "Aucune sortie configurée : ouvre les Réglages";
  const errs = cast.outputs.filter((o) => o.status && o.status.detail);
  $("#cast-summary").textContent = cast.casting
    ? `Diffusion : ${live}/${active.length} en direct${cast.song ? " · " + cast.song : ""}${errs.length ? " · " + errs[0].name + " : " + errs[0].status.detail : ""}`
    : "";
  if (settings) renderOutList();
}

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
    $(".s", el).textContent = (o.enabled ? STATUS_LABEL[state] : "Désactivée") + (st && st.detail ? " — " + st.detail : "") + (o.has_password ? "" : " · mot de passe à saisir");
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
}

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
  await listen("blast://cast", (e) => {
    cast = e.payload;
    renderCast();
  });
  await loadThemes();
  await setupOsDrop();
  renderPlaylist(0);
  send({ type: "refresh" });
})();
