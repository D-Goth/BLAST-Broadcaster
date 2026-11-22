import { $, S, send, ico, hms } from "./core.js";
import { openSettings, renderOutList } from "./settings.js";

export const STATUS_LABEL = { off: "Arrêtée", connecting: "Connexion…", live: "En direct", retrying: "Reconnexion", error: "Erreur" };
export let cast = { casting: false, song: "", outputs: [] };

export const statusOf = (o) => (o && o.status && o.status.state) || "off";

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
  btn.title = cast.outputs.length ? cast.outputs.map(outputLine).join("\n") : "Aucune sortie configurée : ouvre les Réglages";
  const errs = cast.outputs.filter((o) => o.status && o.status.detail);
  $("#cast-summary").textContent = cast.casting
    ? `Diffusion : ${live}/${active.length} en direct${cast.song ? " · " + cast.song : ""}${errs.length ? " · " + errs[0].name + " : " + errs[0].status.detail : ""}`
    : "";
  if (S.settings) renderOutList();
}

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
  $("#cast-btn").addEventListener("click", () => {
    if (!cast.outputs.length) openSettings("cast");
    else send({ type: cast.casting ? "cast_stop" : "cast_start" });
  });
  setInterval(renderCastTimer, 1000);
}
