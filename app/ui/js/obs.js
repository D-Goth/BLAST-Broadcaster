import { $, $$, send, store, hms } from "./core.js";

const OBS_STATUS = { off: "Déconnecté", connecting: "Connexion…", connected: "Connecté", retrying: "Reconnexion…", error: "Erreur" };
let obs = { status: "off", scenes: [], inputs: [] };
let input = null;
let volHeld = false;

const isIdle = () => obs.status === "off" || obs.status === "error";

export function showFx(page) {
  $$(".fx-tab[data-fx]").forEach((t) => t.classList.toggle("active", t.dataset.fx === page));
  $$("#soundfx [data-fxpage]").forEach((el) => (el.hidden = el.dataset.fxpage !== page));
  store.set("blast.fxtab", page);
}

function renderTimers() {
  const now = Date.now();
  const live = $("#obs-live");
  const streaming = !!(obs.stream && obs.stream_since);
  live.classList.toggle("hidden", !streaming);
  if (streaming) $("span", live).textContent = hms(now - obs.stream_since);
  const rec = $("#obs-rec");
  const recording = !!(obs.record && obs.record_since);
  rec.classList.toggle("hidden", !recording);
  rec.classList.toggle("paused", !!obs.record_paused);
  if (recording && !obs.record_paused) $("span", rec).textContent = hms(now - obs.record_since);
  $("use", rec).setAttribute("href", obs.record_paused ? "#i-pause" : "#i-rec");
}

function renderControls(connected) {
  const st = obs.status;
  $("#obs-pill").className = `obs-pill ${st}`;
  $("#obs-pill").title = `OBS : ${OBS_STATUS[st] || st}${obs.detail ? " — " + obs.detail : ""}`;
  $(".fx-tab[data-fx='obs']").classList.toggle("obs-on", connected);
  $("#obs-dot").className = `dot ${connected ? "live" : st === "error" ? "error" : st === "off" ? "" : "connecting"}`;
  $("#obs-status-text").textContent = connected ? `Connecté — OBS ${obs.obs_version || ""}` : `${OBS_STATUS[st] || st}${obs.detail ? " : " + obs.detail : ""}`;
  $("#obs-status-text").title = obs.detail || "";
  $("#obs-connect").textContent = isIdle() ? "Connecter" : "Déconnecter";
  $$(".obs-grid .hw").forEach((b) => (b.disabled = !connected));
  $("#obs-pause").disabled = !connected || !obs.record;
  $("#obs-stream").classList.toggle("on-stream", !!obs.stream);
  $("#obs-stream small").textContent = obs.stream ? "STOP STREAM" : "STREAM";
  $("#obs-record").classList.toggle("on-rec", !!obs.record);
  $("#obs-record small").textContent = obs.record ? "STOP REC" : "REC";
  $("#obs-pause").classList.toggle("on-warn", !!obs.record_paused);
  $("#obs-pause small").textContent = obs.record_paused ? "REPRENDRE" : "PAUSE REC";
}

function renderScenes(connected) {
  const box = $("#obs-scenes");
  box.innerHTML = "";
  if (!connected || !obs.scenes.length) {
    const e = document.createElement("div");
    e.className = "obs-empty";
    e.textContent = connected ? "Aucune scène dans OBS" : "Connecte OBS pour afficher tes scènes";
    box.appendChild(e);
    return;
  }
  for (const name of obs.scenes) {
    const b = document.createElement("button");
    b.className = "scene" + (name === obs.current ? " current" : "");
    b.textContent = name;
    b.title = name;
    b.addEventListener("click", () => send({ type: "obs", action: "scene", name }));
    box.appendChild(b);
  }
}

function renderInputs(connected) {
  const sel = $("#obs-input");
  const names = obs.inputs.map((i) => i.name);
  if (!names.includes(input)) input = names[0] || null;
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
  sel.value = input || "";
  sel.disabled = !connected || !names.length;
  const cur = obs.inputs.find((i) => i.name === input);
  $("#obs-vol").disabled = !cur;
  if (!volHeld) $("#obs-vol").value = cur ? Math.round(cur.volume * 100) : 0;
  $("#obs-vol-v").textContent = cur ? `${Math.round(cur.volume * 100)}%` : "–";
  $("#obs-mute").disabled = !cur;
  $("#obs-mute").classList.toggle("on-warn", !!(cur && cur.muted));
}

export function setObs(o) {
  obs = o;
  render();
}

function render() {
  const connected = obs.status === "connected";
  renderControls(connected);
  renderScenes(connected);
  renderInputs(connected);
  renderTimers();
}

export function init() {
  $$(".fx-tab[data-fx]").forEach((t) => t.addEventListener("click", () => showFx(t.dataset.fx)));
  $("#obs-pill").addEventListener("click", () => {
    showFx("obs");
    if (isIdle()) send({ type: "obs", action: "connect" });
  });
  $("#obs-connect").addEventListener("click", () => send({ type: "obs", action: isIdle() ? "connect" : "disconnect" }));
  $$(".obs-grid [data-obs]").forEach((b) => b.addEventListener("click", () => send({ type: "obs", action: b.dataset.obs })));
  $("#obs-mute").addEventListener("click", () => {
    if (input) send({ type: "obs", action: "mute", input });
  });
  $("#obs-input").addEventListener("change", (e) => {
    input = e.target.value || null;
    render();
  });
  const vol = $("#obs-vol");
  let pending = false;
  vol.addEventListener("pointerdown", () => (volHeld = true));
  document.addEventListener("pointerup", () => (volHeld = false));
  vol.addEventListener("input", (e) => {
    $("#obs-vol-v").textContent = `${e.target.value}%`;
    if (pending || !input) return;
    pending = true;
    setTimeout(() => {
      pending = false;
      send({ type: "obs", action: "volume", input, mul: Number(vol.value) / 100 });
    }, 60);
  });
  setInterval(renderTimers, 500);
  render();
  showFx(store.get("blast.fxtab") === "obs" ? "obs" : "pads");
}
