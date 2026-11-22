import { $, $$, S, T, send, toast, fmt, ico, invoke, confirmClick, pickAudio } from "./core.js";
import { freeDeck } from "./decks.js";

let history = { rows: [], stats: null, current: null };

// [UI-26]
export function renderPlaylist(total) {
  const rows = $("#rows");
  rows.innerHTML = "";
  S.entries.forEach((e, i) => {
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
    rows.appendChild(row);
  });
  const n = S.entries.length;
  $("#playlist-info").textContent = n === 0 ? "0 titre" : `${n} titre${n > 1 ? "s" : ""} · ${fmt(total)}`;
  $("#empty").classList.toggle("hidden", n > 0 || $("#rows").hidden);
}

// [UI-27]
function showPlaylistTab(tab) {
  $$("[data-ptab]").forEach((t) => t.classList.toggle("active", t.dataset.ptab === tab));
  $$("#playlist [data-ppage]").forEach((el) => (el.hidden = el.dataset.ppage !== tab));
  if (tab === "playlist") $("#empty").classList.toggle("hidden", S.entries.length > 0);
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
  const box = $("#hrows");
  box.innerHTML = "";
  if (!history.rows.length) {
    const e = document.createElement("div");
    e.className = "hempty";
    e.textContent = "Aucune diffusion enregistrée pour l'instant : chaque titre passé à l'antenne apparaîtra ici.";
    box.appendChild(e);
  }
  for (const r of history.rows) {
    const el = document.createElement("div");
    const current = r.id === history.current;
    el.className = "hrow hrow-grid" + (current ? " current" : "");
    el.title = r.path;
    el.innerHTML = `<span class="t"></span><span class="ti"></span><span class="ar muted"></span><span class="d"></span><span class="a"><span class="air-dot"></span></span><span class="n"></span>`;
    $(".t", el).textContent = clockTime(r.started_at);
    $(".ti", el).textContent = r.title || "—";
    $(".ar", el).textContent = r.artist;
    $(".d", el).textContent = current ? "en cours" : r.duration_played != null ? fmt(r.duration_played) : "--:--";
    $(".air-dot", el).classList.toggle("on", r.on_air);
    $(".air-dot", el).title = r.on_air ? "Diffusé pendant une diffusion en direct" : "Joué hors diffusion";
    $(".n", el).textContent = r.listeners != null ? r.listeners : "–";
    box.appendChild(el);
  }
  const s = history.stats;
  $("#history-info").textContent = s ? `${s.plays} diffusion${s.plays > 1 ? "s" : ""} · ${s.tracks} titre${s.tracks > 1 ? "s" : ""} en bibliothèque` : "";
}

// [UI-30]
async function exportHistory() {
  const d = new Date();
  const day = [d.getFullYear(), d.getMonth() + 1, d.getDate()].map((n) => String(n).padStart(2, "0")).join("-");
  const path = await T.dialog.save({ title: "Exporter l'historique", defaultPath: `blast-historique-${day}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return;
  try {
    const n = await invoke("export_history", { path });
    toast("info", `${n} diffusion${n > 1 ? "s" : ""} exportée${n > 1 ? "s" : ""} : ${path}`);
  } catch (e) {
    toast("error", String(e));
  }
}

export function init() {
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
  $("#clear").addEventListener("click", () => {
    if (S.entries.length) send({ type: "clear" });
  });
  $("#add-files").addEventListener("click", async () => {
    const paths = await pickAudio(true, "Ajouter des titres");
    if (paths.length) send({ type: "add_paths", paths, index: null });
  });
  $("#add-folder").addEventListener("click", async () => {
    const picked = await T.dialog.open({ multiple: true, directory: true, title: "Ajouter un dossier" });
    const paths = picked == null ? [] : [].concat(picked);
    if (paths.length) send({ type: "add_paths", paths, index: null });
  });
  $$("[data-ptab]").forEach((t) => t.addEventListener("click", () => showPlaylistTab(t.dataset.ptab)));
  $("#history-export").addEventListener("click", exportHistory);
  $("#history-clear").addEventListener("click", (e) =>
    confirmClick(e.currentTarget, "Effacer", "Confirmer l'effacement", () => send({ type: "history_clear" })),
  );
  showPlaylistTab("playlist");
  renderPlaylist(0);
}
