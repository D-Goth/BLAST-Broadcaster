export const T = window.__TAURI__;
export const invoke = T.core.invoke;
export const listen = T.event.listen;

export const AUDIO_EXT = ["mp3", "mp2", "m4a", "mp4", "aac", "alac", "flac", "ogg", "oga", "wav", "aif", "aiff", "aifc", "caf", "mka", "mkv", "webm"];

export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));
export const ico = (name) => `<svg class="ico"><use href="#i-${name}"/></svg>`;

export const S = {
  state: null,
  entries: [],
  settings: null,
};

export function send(cmd) {
  return invoke("blast", { cmd }).catch((e) => toast("error", String(e)));
}

export function call(name, args) {
  return invoke(name, args).catch((e) => {
    toast("error", String(e));
    throw e;
  });
}

export function toast(level, message, actions = []) {
  const el = document.createElement("div");
  el.className = `toast ${level}`;
  const text = document.createElement("span");
  text.textContent = message;
  el.appendChild(text);
  for (const a of actions) {
    const b = document.createElement("button");
    b.className = "btn ghost";
    b.textContent = a.label;
    b.addEventListener("click", () => {
      a.run();
      el.remove();
    });
    el.appendChild(b);
  }
  $("#toasts").appendChild(el);
  const ttl = actions.length ? 30000 : level === "error" ? 8000 : 5000;
  setTimeout(() => el.remove(), ttl);
  return el;
}

export function fmt(secs) {
  if (!isFinite(secs) || secs < 0) secs = 0;
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

export function hms(ms) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return [Math.floor(s / 3600), Math.floor((s % 3600) / 60), s % 60].map((n) => String(n).padStart(2, "0")).join(":");
}

export function dbWidth(peak) {
  if (peak <= 0.00001) return 0;
  const db = 20 * Math.log10(peak);
  return Math.max(0, Math.min(1, (db + 48) / 48));
}

export const store = {
  get(k) {
    try { return localStorage.getItem(k); } catch (_) { return null; }
  },
  set(k, v) {
    try { localStorage.setItem(k, v); } catch (_) {}
  },
};

export function formValues(form) {
  const v = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    if (el.type === "checkbox") v[el.name] = el.checked;
    else if (el.type === "number") v[el.name] = Number(el.value);
    else v[el.name] = el.value;
  }
  return v;
}

export function fillForm(form, data) {
  for (const el of form.elements) {
    if (!el.name || !(el.name in data)) continue;
    if (el.type === "checkbox") el.checked = !!data[el.name];
    else el.value = String(data[el.name]);
  }
}

export function setResult(id, kind, text) {
  const el = $(id);
  el.className = `form-result ${kind}`;
  el.textContent = text;
}

export function fillSelect(sel, items, value) {
  sel.innerHTML = "";
  for (const [id, label] of items) {
    const o = document.createElement("option");
    o.value = id;
    o.textContent = label;
    sel.appendChild(o);
  }
  sel.value = value;
}

export function confirmClick(btn, label, confirmLabel, run) {
  if (btn.dataset.confirm !== "1") {
    btn.dataset.confirm = "1";
    btn.textContent = confirmLabel;
    setTimeout(() => {
      btn.dataset.confirm = "";
      btn.textContent = label;
    }, 4000);
    return;
  }
  btn.dataset.confirm = "";
  btn.textContent = label;
  run();
}

export async function pickAudio(multiple, title) {
  const picked = await T.dialog.open({ multiple, directory: false, title, filters: [{ name: "Audio", extensions: AUDIO_EXT }] });
  return picked == null ? [] : [].concat(picked);
}
