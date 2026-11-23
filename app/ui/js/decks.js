import { $, S, send, fmt, dbWidth, invoke } from "./core.js";
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
// [UI-139]
const deckHold = { A: [0, 0], B: [0, 0] };
const volHeld = { A: false, B: false };
const dbText = (db) => (db <= -60 ? "−∞" : `${db > 0 ? "+" : ""}${Number(db).toFixed(db % 1 ? 1 : 0)} dB`);
let crossfaderHeld = false;
// [UI-145]
const pitchHeld = { A: false, B: false };
const pitchText = (p) => `${p > 0 ? "+" : p < 0 ? "−" : ""}${Math.abs(p).toFixed(1).replace(".", ",")} %`;

// [UI-149]
function renderCue(id, el, st) {
  const c = st.cue;
  const on = !!(c && c.decks[id === "A" ? 0 : 1]);
  el.classList.toggle("cueing", on);
  const where = c && c.device ? `casque : ${c.device}` : "même sortie : antenne à gauche, pré-écoute à droite";
  $(".cue-btn", el).title = `Pré-écoute (CUE) de la platine ${id}${on ? " — active" : ""} · ${where}`;
}

// [UI-146]
function renderPitch(id, el, st) {
  const p = st.deck_pitch ? st.deck_pitch[id === "A" ? 0 : 1] : 0;
  el.classList.toggle("pitched", p !== 0);
  if (pitchHeld[id] || !Number.isFinite(p)) return;
  $(".pitch", el).value = p;
  $(".pitch-v", el).textContent = pitchText(p);
}

// [UI-140]
function renderDeckLevel(id, el, st) {
  const i = id === "A" ? 0 : 1;
  const peak = (st.deck_peak && st.deck_peak[i]) || [0, 0];
  const hold = deckHold[id];
  hold[0] = Math.max(dbWidth(peak[0]), hold[0] * 0.85);
  hold[1] = Math.max(dbWidth(peak[1]), hold[1] * 0.85);
  $(".deck-vu-l", el).style.width = `${(hold[0] * 100).toFixed(1)}%`;
  $(".deck-vu-r", el).style.width = `${(hold[1] * 100).toFixed(1)}%`;
  const db = st.deck_gain_db ? st.deck_gain_db[i] : 0;
  if (!volHeld[id] && Number.isFinite(db)) {
    $(".deck-vol", el).value = db;
    $(".deck-vol-v", el).textContent = dbText(db);
  }
}

// [UI-141]
function saveDeckGains() {
  const gains = ["A", "B"].map((id) => Number($(".deck-vol", decks[id]).value));
  invoke("save_deck_gains", { gains }).catch(() => {});
}

// [UI-142]
const coverShown = { A: undefined, B: undefined };
const coverUrl = { A: null, B: null };

// [UI-143]
function renderCover(id, el, d) {
  const key = d.cover ?? null;
  if (coverShown[id] === key) return;
  coverShown[id] = key;
  const box = $(".deck-cover", el);
  const img = $("img", box);
  const drop = () => {
    if (coverUrl[id]) URL.revokeObjectURL(coverUrl[id]);
    coverUrl[id] = null;
    img.removeAttribute("src");
    box.classList.remove("has");
  };
  if (key === null) return drop();
  invoke("deck_cover", { deck: id })
    .then((buf) => {
      if (coverShown[id] !== key) return;
      drop();
      if (!buf || !buf.byteLength) return;
      coverUrl[id] = URL.createObjectURL(new Blob([buf]));
      img.src = coverUrl[id];
      box.classList.add("has");
    })
    .catch(drop);
}

// [UI-144]
function zoomCover(id) {
  if (!coverUrl[id]) return;
  const layer = document.createElement("div");
  layer.className = "cover-zoom";
  const img = document.createElement("img");
  img.src = coverUrl[id];
  layer.append(img);
  const close = () => {
    layer.remove();
    document.removeEventListener("keydown", onKey, true);
  };
  const onKey = (e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  };
  layer.addEventListener("click", close);
  document.addEventListener("keydown", onKey, true);
  document.body.append(layer);
}

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
  $(".deck-format", el).textContent = d.format || "\u00a0";
  renderCover(id, el, d);
  renderDeckLevel(id, el, st);
  renderPitch(id, el, st);
  renderCue(id, el, st);
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
  const key = clock ? `${clock.id}|${clock.name}|${clock.color}|${clock.program}` : "";
  if (title.dataset.clock === key) return;
  title.dataset.clock = key;
  title.textContent = "MIX";
  title.title = clock ? `${clock.program ? "Émission" : "Grille"} de l'heure : ${clock.name}` : "Aucune grille pour cette heure : l'auto-mix suit la playlist";
  if (!clock) return;
  const tag = document.createElement("span");
  tag.className = "clock-tag";
  tag.style.setProperty("--clock-color", clock.color || "var(--accent)");
  tag.textContent = clock.program ? `\u25B6 ${clock.name}` : clock.name;
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
    const vol = $(".deck-vol", el);
    vol.addEventListener("pointerdown", () => (volHeld[id] = true));
    vol.addEventListener("input", () => {
      $(".deck-vol-v", el).textContent = dbText(Number(vol.value));
      send({ type: "deck_gain", deck: id, db: Number(vol.value) });
    });
    vol.addEventListener("change", () => {
      volHeld[id] = false;
      saveDeckGains();
    });
    vol.addEventListener("dblclick", () => {
      vol.value = 0;
      $(".deck-vol-v", el).textContent = dbText(0);
      send({ type: "deck_gain", deck: id, db: 0 });
      saveDeckGains();
    });
    $(".cue-btn", el).addEventListener("click", () => {
      const c = S.state && S.state.cue;
      send({ type: "deck_cue", deck: id, on: !(c && c.decks[id === "A" ? 0 : 1]) });
    });
    // [UI-147]
    const pitch = $(".pitch", el);
    const setPitch = (p) => {
      $(".pitch-v", el).textContent = pitchText(p);
      send({ type: "deck_pitch", deck: id, percent: p });
    };
    const resetPitch = () => {
      pitch.value = 0;
      setPitch(0);
    };
    pitch.addEventListener("pointerdown", () => (pitchHeld[id] = true));
    pitch.addEventListener("input", () => setPitch(Number(pitch.value)));
    pitch.addEventListener("change", () => (pitchHeld[id] = false));
    pitch.addEventListener("dblclick", resetPitch);
    $(".pitch-v", el).addEventListener("click", resetPitch);
    $(".deck-cover", el).addEventListener("click", () => zoomCover(id));
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
