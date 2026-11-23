import { $, S, send, ico, hms, toast } from "./core.js";
import { openSettings, renderOutList } from "./settings.js";

// [UI-37]
export const STATUS_LABEL = { off: "Arrêtée", connecting: "Connexion…", live: "En direct", retrying: "Reconnexion", error: "Erreur" };
export let cast = { casting: false, song: "", outputs: [] };

export const statusOf = (o) => (o && o.status && o.status.state) || "off";

// [UI-38]
function castLabel(states, live, total) {
  if (live > 0) return `EN DIRECT ${live}/${total}`;
  if (states.includes("error")) return "ERREUR";
  if (states.includes("retrying")) return "RECONNEXION…";
  return "CONNEXION…";
}

function castClass(states, live, total) {
  if (states.includes("error")) return "error";
  if (live === total && live > 0) return "live";
  if (states.includes("retrying")) return "retrying";
  return "connecting";
}

function outputLine(o) {
  const count = o.listeners != null ? ` · ${o.listeners} auditeur(s), max ${o.peak || o.listeners}` : "";
  const detail = o.status && o.status.detail ? " — " + o.status.detail : "";
  return `${o.name} (${o.target}) : ${STATUS_LABEL[statusOf(o)]}${count}${detail}`;
}

// [UI-39]
export function setCast(c) {
  cast = c;
  const btn = $("#cast-btn");
  const active = cast.outputs.filter((o) => o.enabled);
  const states = active.map(statusOf);
  const live = states.filter((s) => s === "live").length;
  btn.className = "cast-btn";
  if (cast.casting) btn.classList.add(castClass(states, live, active.length));
  $("#cast-label").textContent = cast.casting ? castLabel(states, live, active.length) : "DIFFUSER";
  renderCastTimer();
  const lis = $("#cast-listeners");
  const hasCount = cast.casting && cast.listeners != null;
  lis.classList.toggle("hidden", !hasCount);
  if (hasCount) {
    const peak = Math.max(cast.session_peak || 0, ...cast.outputs.map((o) => o.peak || 0));
    lis.innerHTML = `${ico("user")} <b>${cast.listeners}</b> · max ${peak}`;
    lis.title = "Auditeurs en cours · pic d'auditeurs (serveur et session)";
  }
  btn.title = cast.outputs.length ? cast.outputs.map(outputLine).join("\n") : "Aucune sortie configurée : ajoute-en une dans Réglages → Diffusion";
  const errs = cast.outputs.filter((o) => o.status && o.status.detail);
  $("#cast-summary").textContent = cast.casting
    ? `Diffusion : ${live}/${active.length} en direct${cast.song ? " · " + cast.song : ""}${errs.length ? " · " + errs[0].name + " : " + errs[0].status.detail : ""}`
    : "";
  if (S.settings) renderOutList();
  renderCastPanel();
}

// [UI-131]
function toggleCast() {
  if (!cast.outputs.length) {
    toast("warn", "Aucune sortie de diffusion : ajoute-en une dans Réglages → Diffusion.");
    return;
  }
  send({ type: cast.casting ? "cast_stop" : "cast_start" });
}

const panelOpen = () => !$("#cast-panel").classList.contains("hidden");

// [UI-132]
function renderCastPanel() {
  if (!panelOpen()) return;
  const active = cast.outputs.filter((o) => o.enabled);
  const live = active.filter((o) => statusOf(o) === "live").length;
  $("#cp-state").textContent = cast.casting ? `En direct sur ${live}/${active.length} sortie${active.length > 1 ? "s" : ""}` : "Diffusion arrêtée";
  $("#cp-time").textContent = cast.casting && cast.since ? hms(Date.now() - cast.since) : "";
  $("#cp-song").textContent = cast.casting && cast.song ? `♪ ${cast.song}` : "";
  const box = $("#cp-outputs");
  box.innerHTML = "";
  if (!cast.outputs.length) box.innerHTML = `<div class="muted">Aucune sortie configurée.</div>`;
  for (const o of cast.outputs) {
    const row = document.createElement("div");
    row.className = "cp-out" + (o.enabled ? "" : " off");
    const state = o.enabled ? statusOf(o) : "off";
    row.innerHTML = `<span class="dot ${state}"></span><div class="cp-main"><b></b><small class="muted"></small></div><span class="cp-count"></span>`;
    $("b", row).textContent = o.name;
    $("small", row).textContent = o.enabled ? `${o.target} · ${STATUS_LABEL[state]}${o.status && o.status.detail ? " — " + o.status.detail : ""}` : `${o.target} · désactivée`;
    $(".cp-count", row).innerHTML = o.listeners != null ? `${ico("user")} ${o.listeners} <span class="muted">/ max ${o.peak || o.listeners}</span>` : "";
    box.appendChild(row);
  }
  const peak = Math.max(cast.session_peak || 0, ...cast.outputs.map((o) => o.peak || 0));
  $("#cp-total").textContent = cast.casting && cast.listeners != null ? `Total : ${cast.listeners} auditeur(s) · pic de la session ${peak}` : "";
  const t = $("#cp-toggle");
  t.textContent = cast.casting ? "Arrêter la diffusion" : "Démarrer la diffusion";
  t.classList.toggle("accent", !cast.casting);
  t.classList.toggle("danger", cast.casting);
  t.disabled = !cast.outputs.length;
}

function openCastPanel() {
  const p = $("#cast-panel");
  if (panelOpen()) {
    p.classList.add("hidden");
    return;
  }
  p.classList.remove("hidden");
  const r = $(".cast-group").getBoundingClientRect();
  p.style.left = `${Math.max(8, r.left)}px`;
  p.style.top = `${r.bottom + 6}px`;
  renderCastPanel();
}

// [UI-40]
function renderCastTimer() {
  const t = $("#cast-time");
  const on = cast.casting && cast.since;
  t.classList.toggle("hidden", !on);
  if (on) {
    t.textContent = hms(Date.now() - cast.since);
    t.title = "Durée de diffusion";
  }
}

export function init() {
  $("#cast-btn").addEventListener("click", toggleCast);
  $("#cast-more").addEventListener("click", openCastPanel);
  $("#cp-toggle").addEventListener("click", toggleCast);
  $("#cp-settings").addEventListener("click", () => {
    $("#cast-panel").classList.add("hidden");
    openSettings("cast");
  });
  document.addEventListener("pointerdown", (e) => {
    if (!e.target.closest("#cast-panel") && !e.target.closest("#cast-more")) $("#cast-panel").classList.add("hidden");
  });
  setInterval(() => {
    renderCastTimer();
    if (panelOpen() && cast.casting) $("#cp-time").textContent = hms(Date.now() - cast.since);
  }, 1000);
}
