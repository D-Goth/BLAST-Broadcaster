import { $, S, send, fmt, dbWidth } from "./core.js";
import { view } from "./theme.js";
import { renderVoice } from "./voice.js";
import { renderPadStates } from "./pads.js";

// [UI-21]
const STATE_LABEL = {
  empty: "VIDE",
  loading: "CHARGEMENT",
  loaded: "PRÊTE",
  playing: "LECTURE",
  paused: "PAUSE",
  finished: "TERMINÉ",
};

const decks = {};
const meterHold = { l: 0, r: 0, red: 0, clipUntil: 0 };
let crossfaderHeld = false;

// [UI-22]
function renderDeck(id, d) {
  const el = decks[id];
  const st = S.state;
  el.classList.toggle("playing", d.state === "playing");
  el.classList.toggle("on-air", st.on_air === id);
  el.classList.toggle("rainbow", view.rainbow && st.on_air === id);
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
  const zone = st.automix && d.duration > 0 ? Math.min(1, st.fade / d.duration) : 0;
  $(".fade-zone", el).style.width = `${(zone * 100).toFixed(2)}%`;
  const has = d.state !== "empty" && d.state !== "loading";
  $('[data-action="play"]', el).disabled = !has || d.state === "playing";
  $('[data-action="pause"]', el).disabled = d.state !== "playing";
  $('[data-action="stop"]', el).disabled = !has;
  $('[data-action="eject"]', el).disabled = !has || d.state === "playing";
  $('[data-action="next"]', el).disabled = d.state === "loading";
}

// [UI-23]
function renderMeters(st) {
  const now = performance.now();
  meterHold.l = Math.max(dbWidth(st.peak[0]), meterHold.l * 0.85);
  meterHold.r = Math.max(dbWidth(st.peak[1]), meterHold.r * 0.85);
  $("#meter-l").style.width = `${(meterHold.l * 100).toFixed(1)}%`;
  $("#meter-r").style.width = `${(meterHold.r * 100).toFixed(1)}%`;
  meterHold.red = Math.max(Math.max(0, st.reduction || 0), meterHold.red * 0.8);
  $("#meter-red").style.width = `${Math.min(100, (meterHold.red / 12) * 100).toFixed(1)}%`;
  $("#red-v").textContent = meterHold.red >= 0.1 ? `-${meterHold.red.toFixed(1)} dB` : "0 dB";
  if (Math.max(st.peak[0], st.peak[1]) >= 0.999) meterHold.clipUntil = now + 1500;
  $("#clip").classList.toggle("on", now < meterHold.clipUntil);
}

// [UI-24]
// [UI-99]
function renderClock(clock) {
  const title = $(".mixer-title");
  const key = clock ? `${clock.id}|${clock.name}|${clock.color}` : "";
  if (title.dataset.clock === key) return;
  title.dataset.clock = key;
  title.textContent = "MIX";
  title.title = clock ? `Grille de l'heure : ${clock.name}` : "Aucune grille pour cette heure : l'auto-mix suit la playlist";
  if (!clock) return;
  const tag = document.createElement("span");
  tag.className = "clock-tag";
  tag.style.setProperty("--clock-color", clock.color || "var(--accent)");
  tag.textContent = clock.name;
  title.append(" · ", tag);
}

export function renderState() {
  const st = S.state;
  if (!st) return;
  renderDeck("A", st.decks[0]);
  renderDeck("B", st.decks[1]);
  $("#onair").classList.toggle("live", !!st.on_air);
  $("#onair").textContent = st.on_air ? `ON AIR · ${st.on_air}` : "ON AIR";
  if (!crossfaderHeld) $("#crossfader").value = Math.round(st.crossfader * 1000);
  const target = st.crossfader < 0.5 ? "B" : "A";
  $("#fade-btn").innerHTML = `FONDU &rarr; ${target}`;
  $("#fade-btn").dataset.target = target;
  $("#automix").classList.toggle("on", st.automix);
  $("#automix").textContent = st.automix ? "AUTO-MIX ACTIF" : "AUTO-MIX";
  renderClock(st.clock);
  if (document.activeElement !== $("#fade-seconds")) $("#fade-seconds").value = st.fade;
  renderMeters(st);
  $("#device").textContent = st.audio_error ? "Aucune sortie audio" : `Sortie : ${st.device} · ${st.sample_rate} Hz`;
  $("#audio-error").textContent = st.audio_error ? `Audio indisponible : ${st.audio_error}` : "";
  renderVoice();
  renderPadStates();
}

// [UI-25]
export function freeDeck() {
  const st = S.state;
  if (!st) return "A";
  const free = (d) => d.state !== "playing" && d.state !== "loading";
  const order = st.on_air === "A" ? ["B", "A"] : ["A", "B"];
  for (const id of order) {
    const d = st.decks[id === "A" ? 0 : 1];
    if (free(d) && (d.state === "empty" || d.state === "finished")) return id;
  }
  for (const id of order) {
    if (free(st.decks[id === "A" ? 0 : 1])) return id;
  }
  return null;
}

export function init() {
  decks.A = $('.deck[data-deck="A"]');
  decks.B = $('.deck[data-deck="B"]');
  for (const [id, el] of Object.entries(decks)) {
    el.addEventListener("click", (e) => {
      const btn = e.target.closest("[data-action]");
      if (btn) send({ type: btn.dataset.action, deck: id });
    });
    $(".progress", el).addEventListener("click", (e) => {
      const r = e.currentTarget.getBoundingClientRect();
      send({ type: "seek", deck: id, fraction: (e.clientX - r.left) / r.width });
    });
  }
  const xf = $("#crossfader");
  let pending = false;
  xf.addEventListener("pointerdown", () => (crossfaderHeld = true));
  document.addEventListener("pointerup", () => (crossfaderHeld = false));
  xf.addEventListener("input", () => {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => {
      pending = false;
      send({ type: "crossfader", value: Number(xf.value) / 1000 });
    });
  });
  $("#fade-btn").addEventListener("click", (e) => send({ type: "crossfade", to: e.currentTarget.dataset.target || "B" }));
  $("#automix").addEventListener("click", () => send({ type: "auto_mix", enabled: !(S.state && S.state.automix) }));
  $("#fade-seconds").addEventListener("change", (e) => send({ type: "fade_seconds", value: Number(e.target.value) || 0 }));
}
