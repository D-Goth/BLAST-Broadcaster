import { $, $$, S, T, send, toast, fmt, ico, invoke, confirmClick, AUDIO_EXT } from "./core.js";
import { freeDeck } from "./decks.js";
import { virtualList } from "./vlist.js";

const KIND_LABEL = { morceau: "Morceau", jingle: "Jingle", pub: "Pub" };
let history = { all: [], morceau: [], jingle: [], pub: [], stats: null, current: null, capacity: 200 };
let filter = "all";

// [UI-26]
let playlistView = null;
let historyView = null;

// [UI-103]
function playlistRow(i) {
  const e = S.entries[i];
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
  if (e.slot) {
    const tag = document.createElement("span");
    tag.className = "slot-tag";
    tag.style.setProperty("--slot-color", e.slot.color || "var(--accent)");
    tag.textContent = e.slot.name;
    tag.title = "Ajouté par la grille";
    $(".title", row).prepend(tag);
  }
  if (e.kind && e.kind !== "morceau") {
    const tag = document.createElement("span");
    tag.className = `kind-tag ${e.kind}`;
    tag.textContent = KIND_LABEL[e.kind];
    $(".title", row).prepend(tag);
  }
  if (e.blocked && !e.error) {
    row.classList.add("blocked");
    const w = document.createElement("span");
    w.className = "wait";
    w.innerHTML = ico("clock");
    w.title = `En attente : ${e.blocked}`;
    $(".title", row).prepend(w);
    row.title = `${row.title}\nEn attente (règles de séparation) : ${e.blocked}`;
  }
  $(".artist", row).textContent = e.artist;
  return row;
}

export function renderPlaylist(total) {
  playlistView.setCount(S.entries.length);
  const n = S.entries.length;
  $("#playlist-info").textContent = n === 0 ? "0 titre" : `${n} titre${n > 1 ? "s" : ""} · ${fmt(total)}`;
  $("#empty").classList.toggle("hidden", n > 0 || $("#rows").hidden);
}

// [UI-104]
export function setBlocked(list) {
  const reasons = new Map(list);
  for (const e of S.entries) e.blocked = reasons.get(e.id) || null;
  playlistView.refresh();
}

// [UI-27]
function showPlaylistTab(tab) {
  $$("[data-ptab]").forEach((t) => t.classList.toggle("active", t.dataset.ptab === tab));
  $$("#playlist [data-ppage]").forEach((el) => (el.hidden = el.dataset.ppage !== tab));
  if (tab === "playlist") $("#empty").classList.toggle("hidden", S.entries.length > 0);
  if (tab === "playlist") playlistView.refresh();
  if (tab === "history") historyView.refresh();
  document.dispatchEvent(new CustomEvent("blast:ptab", { detail: tab }));
}

// [UI-28]
function clockTime(secs) {
  const d = new Date(secs * 1000);
  const hm = d.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  return d.toDateString() === new Date().toDateString() ? hm : `${d.toLocaleDateString("fr-FR", { day: "2-digit", month: "2-digit" })} ${hm}`;
}

// [UI-29]
export function setHistory(h) {
  history = h;
  renderHistory();
}

// [UI-82]
function historyRow(i) {
  const r = (history[filter] || [])[i];
  const el = document.createElement("div");
  const current = r.id === history.current;
  el.className = "hrow hrow-grid" + (current ? " current" : "");
  el.title = r.path;
  el.innerHTML = `<span class="t"></span><span class="k"><span class="kind-tag"></span></span><span class="ti"></span><span class="ar muted"></span><span class="d"></span><span class="a"><span class="air-dot"></span></span><span class="n"></span>`;
  $(".t", el).textContent = clockTime(r.started_at);
  $(".kind-tag", el).textContent = KIND_LABEL[r.kind] || r.kind;
  $(".kind-tag", el).classList.add(r.kind);
  $(".ti", el).textContent = r.title || "—";
  $(".ar", el).textContent = r.artist;
  $(".d", el).textContent = current ? "en cours" : r.duration_played != null ? fmt(r.duration_played) : "--:--";
  $(".air-dot", el).classList.toggle("on", r.on_air);
  $(".air-dot", el).title = r.on_air ? "Diffusé pendant une diffusion en direct" : "Joué hors diffusion";
  $(".n", el).textContent = r.listeners != null ? r.listeners : "–";
  return el;
}

function historyEmpty() {
  const e = document.createElement("div");
  e.className = "hempty";
  e.textContent = filter === "all"
    ? "Aucune diffusion enregistrée pour l'instant : chaque titre passé à l'antenne apparaîtra ici."
    : `Aucun passage de type « ${KIND_LABEL[filter]} » dans les ${history.capacity} derniers.`;
  return e;
}

function renderHistory() {
  historyView.setCount((history[filter] || []).length);
  const s = history.stats;
  $("#history-info").textContent = s ? `${s.plays} passage${s.plays > 1 ? "s" : ""} · ${s.tracks} titre${s.tracks > 1 ? "s" : ""}` : "";
  $("#history-info").title = `Affichage : ${history.capacity} derniers passages par catégorie, gardés en mémoire`;
}

function setFilter(f) {
  filter = f;
  $$("[data-hfilter]").forEach((b) => b.classList.toggle("active", b.dataset.hfilter === f));
  renderHistory();
}

const day = (d) => [d.getFullYear(), d.getMonth() + 1, d.getDate()].map((n) => String(n).padStart(2, "0")).join("-");

// [UI-83]
function openExportMenu(anchor) {
  const menu = $("#export-menu");
  $("#export-kind").value = filter;
  const now = new Date();
  if (!$("#export-from").value && !$("#export-to").value) {
    $("#export-from").value = day(new Date(now.getFullYear(), now.getMonth(), 1));
    $("#export-to").value = day(now);
  }
  menu.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  const m = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(r.left, window.innerWidth - m.width - 8)}px`;
  menu.style.top = `${Math.max(8, r.top - m.height - 6)}px`;
}

const closeExportMenu = () => $("#export-menu").classList.add("hidden");

// [UI-30]
async function exportHistory() {
  const kind = $("#export-kind").value;
  const fromValue = $("#export-from").value;
  const toValue = $("#export-to").value;
  const from = fromValue ? Math.floor(new Date(`${fromValue}T00:00:00`).getTime() / 1000) : null;
  const to = toValue ? Math.floor(new Date(`${toValue}T00:00:00`).getTime() / 1000) + 86400 : null;
  closeExportMenu();
  const suffix = [kind === "all" ? "" : kind, fromValue, toValue].filter(Boolean).join("_") || day(new Date());
  const path = await T.dialog.save({ title: "Exporter l'historique", defaultPath: `blast-historique-${suffix}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return;
  try {
    const n = await invoke("export_history", { path, kind, from, to });
    toast("info", `${n} passage${n > 1 ? "s" : ""} exporté${n > 1 ? "s" : ""} : ${path}`);
  } catch (e) {
    toast("error", String(e));
  }
}

// [UI-100]
function openShuffleMenu(anchor) {
  const menu = $("#shuffle-menu");
  menu.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  const m = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(r.left, window.innerWidth - m.width - 8)}px`;
  menu.style.top = `${r.bottom + 6}px`;
}

const closeShuffleMenu = () => $("#shuffle-menu").classList.add("hidden");

// [UI-105]
async function saveM3u() {
  if (!S.entries.length) return toast("warn", "La playlist est vide.");
  const day = new Date().toISOString().slice(0, 10);
  const path = await T.dialog.save({
    title: "Enregistrer la playlist",
    defaultPath: `blast-playlist-${day}.m3u8`,
    filters: [{ name: "Playlist M3U", extensions: ["m3u8", "m3u"] }],
  });
  if (path) send({ type: "save_playlist", path });
}

export function init() {
  playlistView = virtualList($("#rows"), 36, playlistRow);
  historyView = virtualList($("#hrows"), 32, historyRow, historyEmpty);
  const rows = $("#rows");
  rows.addEventListener("dblclick", (e) => {
    const row = e.target.closest(".row");
    if (!row || e.target.closest("button")) return;
    const deck = freeDeck();
    if (deck) send({ type: "load_entry", id: Number(row.dataset.id), deck });
    else toast("warn", "Aucune platine libre.");
  });
  rows.addEventListener("click", (e) => {
    const btn = e.target.closest("button");
    if (!btn) return;
    const id = Number(btn.closest(".row").dataset.id);
    if (btn.dataset.load) send({ type: "load_entry", id, deck: btn.dataset.load });
    else if ("remove" in btn.dataset) send({ type: "remove", id });
  });
  $("#shuffle").addEventListener("click", (e) => openShuffleMenu(e.currentTarget));
  $$("[data-shuffle]").forEach((b) =>
    b.addEventListener("click", () => {
      closeShuffleMenu();
      const action = b.dataset.shuffle;
      if (action === "dedupe") send({ type: "dedupe" });
      else send({ type: "shuffle", spread: action === "spread" });
    }),
  );
  document.addEventListener("pointerdown", (e) => {
    if (!e.target.closest("#shuffle-menu, #shuffle")) closeShuffleMenu();
  });
  $("#clear").addEventListener("click", () => {
    if (S.entries.length) send({ type: "clear" });
  });
  $("#add-files").addEventListener("click", async () => {
    const picked = await T.dialog.open({
      multiple: true,
      directory: false,
      title: "Ajouter des titres ou une playlist",
      filters: [
        { name: "Audio et playlists", extensions: [...AUDIO_EXT, "m3u", "m3u8"] },
        { name: "Playlists M3U", extensions: ["m3u", "m3u8"] },
      ],
    });
    const paths = picked == null ? [] : [].concat(picked);
    if (paths.length) send({ type: "add_paths", paths, index: null });
  });
  $("#save-m3u").addEventListener("click", saveM3u);
  $("#add-folder").addEventListener("click", async () => {
    const picked = await T.dialog.open({ multiple: true, directory: true, title: "Ajouter un dossier" });
    const paths = picked == null ? [] : [].concat(picked);
    if (paths.length) send({ type: "add_paths", paths, index: null });
  });
  $$("[data-ptab]").forEach((t) => t.addEventListener("click", () => showPlaylistTab(t.dataset.ptab)));
  $("#history-export").addEventListener("click", (e) => openExportMenu(e.currentTarget));
  $("#export-go").addEventListener("click", exportHistory);
  $("#export-all").addEventListener("click", () => {
    $("#export-from").value = "";
    $("#export-to").value = "";
    exportHistory();
  });
  document.addEventListener("pointerdown", (e) => {
    if (!e.target.closest("#export-menu, #history-export")) closeExportMenu();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeExportMenu();
  });
  $$("[data-hfilter]").forEach((b) => b.addEventListener("click", () => setFilter(b.dataset.hfilter)));
  $("#history-clear").addEventListener("click", (e) =>
    confirmClick(e.currentTarget, "Effacer", "Confirmer l'effacement", () => send({ type: "history_clear" })),
  );
  showPlaylistTab("playlist");
  renderPlaylist(0);
}
