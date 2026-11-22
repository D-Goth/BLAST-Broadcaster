import { $, S, send, ico, dbWidth, invoke, toast, fillSelect } from "./core.js";

// [UI-52]
const SIGNED = (v) => `${v > 0 ? "+" : ""}${v} dB`;
const SLIDERS = [
  ["#mic-gain", "mic_gain_db", SIGNED],
  ["#music-gain", "music_gain_db", SIGNED],
  ["#duck-db", "duck_db", (v) => `${v} dB`],
  ["#eq-low", "low_db", SIGNED],
  ["#eq-mid", "mid_db", SIGNED],
  ["#eq-high", "high_db", SIGNED],
];

let voice = null;
let saveTimer = null;
const mic = { latched: false, held: false };

const setMic = (open) => send({ type: "mic_open", open });

// [UI-53]
export function renderVoice() {
  if (!S.state || !S.state.mic) return;
  const m = S.state.mic;
  const on = mic.latched && m.open;
  $("#voicefx").classList.toggle("disabled", !m.available);
  $("#mic-name").title = m.available ? `Micro : ${m.name || "par défaut"}` : m.error ? `Micro indisponible : ${m.error}` : "Aucun micro";
  $("#mic-toggle").classList.toggle("on", on);
  $("#mic-toggle").innerHTML = on ? `${ico("unlock")}<span>OUVERT</span>` : `${ico("lock")}<span>VERROU</span>`;
  $("#ptt").classList.toggle("active", m.open);
  $("#meter-mic").style.width = `${(dbWidth(m.peak) * 100).toFixed(1)}%`;
}

// [UI-54]
function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    invoke("save_voice", { voice }).catch((e) => toast("error", String(e)));
  }, 250);
}

function edit(change) {
  if (!voice) return;
  change(voice);
  saveSoon();
}

// [UI-55]
function applyVoice(v) {
  voice = { ...v };
  for (const [sel, key, label] of SLIDERS) {
    $(sel).value = voice[key];
    $(sel + "-v").textContent = label(voice[key]);
  }
  $("#eq-hpf").checked = !!voice.highpass;
  $("#mic-monitor").checked = !!voice.monitor;
  $("#duck-toggle").classList.toggle("on", !!voice.duck);
}

// [UI-56]
export function toggleLock() {
  mic.latched = !(mic.latched && S.state && S.state.mic.open);
  setMic(mic.latched);
}

// [UI-57]
export function pttDown() {
  if (mic.held) return;
  mic.held = true;
  setMic(true);
}

// [UI-58]
export function pttUp() {
  if (!mic.held) return;
  mic.held = false;
  if (!mic.latched) setMic(false);
}

// [UI-59]
export async function loadVoiceSettings() {
  try {
    const view = await invoke("get_settings");
    fillSelect($("#voice-preset"), view.voice_presets, view.voice.preset);
    fillSelect($("#master-preset"), view.master_presets, view.audio.master_preset);
    applyVoice(view.voice);
    $("#sfx-vol").value = view.soundboard.volume_db;
  } catch (e) {
    toast("error", String(e));
  }
}

export function init() {
  for (const [sel, key, label] of SLIDERS) {
    $(sel).addEventListener("input", (e) =>
      edit((v) => {
        v[key] = Number(e.target.value);
        $(sel + "-v").textContent = label(v[key]);
      }),
    );
  }
  $("#eq-hpf").addEventListener("change", (e) => edit((v) => (v.highpass = e.target.checked)));
  $("#mic-monitor").addEventListener("change", (e) => edit((v) => (v.monitor = e.target.checked)));
  $("#duck-toggle").addEventListener("click", () =>
    edit((v) => {
      v.duck = !v.duck;
      $("#duck-toggle").classList.toggle("on", v.duck);
    }),
  );
  $("#voice-preset").addEventListener("change", (e) => edit((v) => (v.preset = e.target.value)));
  $("#master-preset").addEventListener("change", (e) => {
    invoke("save_master_preset", { preset: e.target.value }).catch((err) => toast("error", String(err)));
  });
  $("#mic-toggle").addEventListener("click", toggleLock);
  const ptt = $("#ptt");
  ptt.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    ptt.setPointerCapture(e.pointerId);
    pttDown();
  });
  ptt.addEventListener("pointerup", pttUp);
  ptt.addEventListener("pointercancel", pttUp);
  window.addEventListener("blur", pttUp);
}
