import { $, $$, S, invoke, toast, formValues, fillForm, setResult } from "./core.js";
import { cast, statusOf, STATUS_LABEL } from "./cast.js";
import { renderAbout } from "./update.js";
import { applySettingsView as applyRotation } from "./rotation.js";

// [UI-41]
const BITRATES = [32, 40, 48, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const KIND_LABEL = { icecast: "Icecast", shoutcast_v1: "Shoutcast v1", shoutcast_v2: "Shoutcast v2" };
const NEW_OUTPUT = {
  id: "", name: "Ma webradio", enabled: true, kind: "icecast", host: "localhost", port: 8000,
  mount: "/live", sid: 1, user: "source", bitrate: 128, mono: false, station: "BLAST Broadcaster",
  genre: "", description: "", url: "", public: false, has_password: false,
};
let selectedOut = null;

// [UI-42]
export function renderOutList() {
  const box = $("#out-items");
  box.innerHTML = "";
  for (const o of S.settings.outputs) {
    const co = cast.outputs.find((x) => x.id === o.id);
    const st = co ? co.status : null;
    const state = statusOf(co);
    const el = document.createElement("div");
    el.className = "out-item" + (o.id === selectedOut ? " active" : "") + (o.enabled ? "" : " disabled");
    el.innerHTML = `<div class="n"><span class="dot ${state}"></span><span></span></div><div class="t"></div><div class="s muted"></div>`;
    $(".n span:last-child", el).textContent = o.name;
    $(".t", el).textContent = `${KIND_LABEL[o.kind]} · ${o.host}:${o.port}${o.kind === "icecast" ? o.mount : ""} · ${o.bitrate} kb/s`;
    const count = co && co.listeners != null ? ` · ${co.listeners} auditeur${co.listeners > 1 ? "s" : ""} (max ${co.peak || co.listeners})` : "";
    $(".s", el).textContent = (o.enabled ? STATUS_LABEL[state] : "Désactivée") + count + (st && st.detail ? " — " + st.detail : "") + (o.has_password ? "" : " · mot de passe à saisir");
    el.addEventListener("click", () => selectOutput(o.id));
    box.appendChild(el);
  }
  const has = S.settings.outputs.length > 0 || selectedOut === "new";
  $("#out-empty").classList.toggle("hidden", has);
  $("#out-body").classList.toggle("hidden", !has);
}

function applyKindFields() {
  const kind = $("#out-form").elements.kind.value;
  $$("#out-form [data-for]").forEach((el) => el.classList.toggle("hidden", el.dataset.for !== kind));
}

// [UI-43]
function selectOutput(id) {
  selectedOut = id;
  const form = $("#out-form");
  const o = S.settings.outputs.find((x) => x.id === id) || NEW_OUTPUT;
  fillForm(form, { ...o, mono: String(o.mono) });
  form.elements.password.value = "";
  form.elements.password.placeholder = o.has_password ? "•••••• enregistré (laisser vide pour garder)" : "mot de passe de la source";
  $("#out-delete").disabled = !o.id;
  setResult("#out-result", "", "");
  applyKindFields();
  renderOutList();
}

// [UI-44]
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

// [UI-45]
function fillDevices(sel, list, def, defLabel, value) {
  sel.innerHTML = "";
  const d = document.createElement("option");
  d.value = "";
  d.textContent = `${defLabel}${def ? " (" + def + ")" : ""}`;
  sel.appendChild(d);
  for (const name of list) {
    const o = document.createElement("option");
    o.value = name;
    o.textContent = name;
    sel.appendChild(o);
  }
  sel.value = value || "";
}

function micStatus() {
  const m = S.state && S.state.mic;
  if (!m) return "";
  if (m.available) return `Micro actif : ${m.name}`;
  return m.error ? `Micro indisponible : ${m.error}` : "Aucun micro détecté.";
}

// [UI-46]
function applySettings(view, keepSelection = true) {
  S.settings = view;
  $("#store-info").textContent = `Mots de passe : ${view.secret_store}`;
  if (view.warning) toast("warn", view.warning);
  if (!keepSelection || !view.outputs.some((o) => o.id === selectedOut)) {
    selectedOut = view.outputs.length ? view.outputs[0].id : null;
  }
  if (selectedOut) selectOutput(selectedOut);
  else renderOutList();
  const chatForm = $("#chat-settings-form");
  fillForm(chatForm, view.chat);
  chatForm.elements.token.value = "";
  chatForm.elements.ssn_session.value = "";
  chatForm.elements.ssn_session.placeholder = view.ssn_has_session ? "•••••• enregistré" : "identifiant ou lien du dock (…dock.html?session=…)";
  chatForm.elements.token.placeholder = view.chat_has_token ? "•••••• enregistré" : "jeton avec les droits chat:read et chat:edit";
  $("#chat-forget").disabled = !view.chat_has_token;
  const aura = $("#aura-form");
  fillForm(aura, view.aura);
  aura.elements.password.value = "";
  aura.elements.password.placeholder = view.aura_has_password ? "•••••• enregistré" : "vide si OBS n'en demande pas";
  fillDevices($("#audio-form").elements.device, view.devices, view.default_device, "Sortie par défaut du système", view.audio.device);
  fillForm($("#automation-form"), {
    ...view.automation.rules,
    history_days: view.automation.history_days,
    memory_size: view.automation.memory_size,
  });
  fillForm($("#categories-form"), {
    jingle_keywords: view.categories.jingle_keywords.join(", "),
    pub_keywords: view.categories.pub_keywords.join(", "),
  });
  $("#library-info").textContent = view.library
    ? `Bibliothèque : ${view.library.tracks} titres connus, ${view.library.plays} diffusions enregistrées. Base : ${view.db_path}`
    : `Base : ${view.db_path}`;
  const mic = $("#mic-form").elements;
  fillDevices(mic.device, view.input_devices, view.default_input, "Micro par défaut du système", view.voice.input_device);
  mic.enabled.checked = !!view.voice.enabled;
  $("#mic-status").textContent = micStatus();
  applyRotation(view);
}

// [UI-47]
async function saveForm(resultId, run) {
  try {
    const view = await run();
    if (view) applySettings(view);
    setResult(resultId, "ok", "Enregistré.");
  } catch (err) {
    setResult(resultId, "err", String(err));
  }
}

// [UI-48]
async function testForm(resultId, busy, run) {
  setResult(resultId, "busy", busy);
  try {
    setResult(resultId, "ok", await run());
  } catch (err) {
    setResult(resultId, "err", String(err));
  }
}

// [UI-138]
function chatValues(forget) {
  const v = formValues($("#chat-settings-form"));
  return {
    chat: {
      twitch_enabled: v.twitch_enabled,
      ssn_enabled: v.ssn_enabled,
      channel: v.channel.trim(),
      bot_login: v.bot_login.trim(),
      autoconnect: v.autoconnect,
      announce: v.announce,
      cmd_np: v.cmd_np,
      cmd_request: v.cmd_request,
      cmd_sfx: v.cmd_sfx,
      request_limit: Math.max(1, Number(v.request_limit) || 2),
    },
    token: forget ? null : v.token || null,
    forgetToken: forget,
    ssnSession: v.ssn_session.trim() || null,
  };
}

function auraValues() {
  const v = formValues($("#aura-form"));
  return {
    aura: { host: v.host.trim(), port: v.port, auto_connect: v.auto_connect, now_playing_source: v.now_playing_source },
    password: v.password || null,
  };
}

function automationValues() {
  const v = formValues($("#automation-form"));
  const n = (x) => Math.max(0, x | 0);
  return {
    rules: { enabled: v.enabled, artist_minutes: n(v.artist_minutes), title_minutes: n(v.title_minutes) },
    history_days: n(v.history_days),
    memory_size: n(v.memory_size),
  };
}

// [UI-84]
function categoryValues() {
  const v = formValues($("#categories-form"));
  const list = (text) => text.split(",").map((w) => w.trim()).filter(Boolean);
  return { jingle_keywords: list(v.jingle_keywords), pub_keywords: list(v.pub_keywords) };
}

// [UI-49]
export async function openSettings(tab) {
  try {
    applySettings(await invoke("get_settings"), false);
  } catch (e) {
    toast("error", String(e));
    return;
  }
  $("#settings").classList.remove("hidden");
  showTab(tab || $(".modal-tabs .tab.active").dataset.tab);
}

export const settingsOpen = () => !$("#settings").classList.contains("hidden");
const closeSettings = () => {
  $("#settings").classList.add("hidden");
  document.dispatchEvent(new CustomEvent("blast:settings-tab", { detail: null }));
};

// [UI-50]
function showTab(tab) {
  $$(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === tab));
  $$(".tab-page").forEach((p) => p.classList.toggle("hidden", p.dataset.page !== tab));
  if (tab === "about") renderAbout();
  document.dispatchEvent(new CustomEvent("blast:settings-tab", { detail: tab }));
}

// [UI-51]
async function deleteOutput() {
  if (!selectedOut || selectedOut === "new") return;
  const btn = $("#out-delete");
  if (btn.dataset.confirm !== selectedOut) {
    btn.dataset.confirm = selectedOut;
    btn.textContent = "Confirmer la suppression";
    setTimeout(() => {
      btn.dataset.confirm = "";
      btn.textContent = "Supprimer";
    }, 4000);
    return;
  }
  btn.dataset.confirm = "";
  btn.textContent = "Supprimer";
  try {
    applySettings(await invoke("delete_output", { id: selectedOut }), false);
  } catch (err) {
    setResult("#out-result", "err", String(err));
  }
}

export function init() {
  const bitrate = $("#out-form").elements.bitrate;
  for (const b of BITRATES) {
    const o = document.createElement("option");
    o.value = b;
    o.textContent = `${b} kb/s`;
    bitrate.appendChild(o);
  }
  $("#settings-btn").addEventListener("click", () => openSettings());
  $("#settings-close").addEventListener("click", closeSettings);
  $("#settings").addEventListener("pointerdown", (e) => {
    if (e.target.id === "settings") closeSettings();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeSettings();
  });
  $$(".tab").forEach((t) => t.addEventListener("click", () => showTab(t.dataset.tab)));
  $("#out-form").elements.kind.addEventListener("change", applyKindFields);
  $("#out-add").addEventListener("click", () => selectOutput("new"));
  $("#out-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const { out, password } = currentOutput();
    saveForm("#out-result", async () => {
      const view = await invoke("save_output", { output: out, password });
      selectedOut = out.id || (view.outputs.length ? view.outputs[view.outputs.length - 1].id : null);
      return view;
    });
  });
  $("#out-test").addEventListener("click", () => {
    const { out, password } = currentOutput();
    testForm("#out-result", "Connexion en cours…", () => invoke("test_output", { output: out, password }));
  });
  $("#out-delete").addEventListener("click", deleteOutput);
  $("#aura-form").addEventListener("submit", (e) => {
    e.preventDefault();
    saveForm("#aura-result", () => invoke("save_aura", auraValues()));
  });
  $("#chat-settings-form").addEventListener("submit", (e) => {
    e.preventDefault();
    saveForm("#chat-result", () => invoke("save_chat", chatValues(false)));
  });
  $("#chat-forget").addEventListener("click", () => saveForm("#chat-result", () => invoke("save_chat", chatValues(true))));
  $("#aura-test").addEventListener("click", () => testForm("#aura-result", "Connexion à OBS…", () => invoke("test_aura", auraValues())));
  $("#audio-form").addEventListener("submit", (e) => {
    e.preventDefault();
    saveForm("#audio-result", () => invoke("save_audio", { device: $("#audio-form").elements.device.value || null }));
  });
  $("#mic-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const f = $("#mic-form").elements;
    saveForm("#mic-result", () => invoke("save_input", { enabled: f.enabled.checked, device: f.device.value || null }));
  });
  $("#automation-form").addEventListener("submit", (e) => {
    e.preventDefault();
    saveForm("#automation-result", () => invoke("save_automation", { automation: automationValues() }));
  });
  $("#categories-form").addEventListener("submit", (e) => {
    e.preventDefault();
    saveForm("#categories-result", () => invoke("save_categories", { categories: categoryValues() }));
  });
}
