import { $, T, send, invoke, toast, fmt } from "./core.js";

let view = { items: [], dirs: [], scanning: false };

const errorOut = (e) => toast("error", String(e));

// [UI-79]
function jingleButton(j) {
  const el = document.createElement("button");
  el.className = "pad jingle" + (j.playing ? " playing" : "") + (!j.ready && !j.error ? " loading" : "") + (j.error ? " error" : "");
  el.dataset.index = j.index;
  el.title = `${j.name}\n${j.path}${j.error ? "\n⚠ " + j.error : ""}\nClic : lancer à l'antenne · reclic : arrêter`;
  const label = document.createElement("span");
  label.textContent = j.name;
  const dur = document.createElement("span");
  dur.className = "key";
  dur.textContent = j.duration != null ? fmt(j.duration) : "";
  el.append(label, dur);
  return el;
}

// [UI-80]
export function setJingles(v) {
  view = v;
  const box = $("#jingles");
  box.innerHTML = "";
  for (const j of view.items) box.appendChild(jingleButton(j));
  if (!view.items.length) {
    const e = document.createElement("div");
    e.className = "obs-empty";
    e.textContent = view.scanning
      ? "Lecture du dossier des jingles…"
      : view.dirs.length
        ? "Aucun son dans le dossier des jingles"
        : "Choisis le dossier de tes jingles (bouton Dossier…)";
    box.appendChild(e);
  }
  const dir = view.dirs[0] || "";
  const name = dir.split(/[\\/]/).filter(Boolean).pop() || "";
  $("#jingle-info").textContent = name ? `${name} · ${view.items.length}` : "";
  $("#jingle-info").title = dir;
}

// [UI-81]
async function pickDir() {
  const picked = await T.dialog.open({ directory: true, multiple: false, title: "Dossier des jingles" });
  if (!picked) return;
  invoke("save_jingles", { dirs: [].concat(picked), volumeDb: null }).catch(errorOut);
}

export function applySettings(settings) {
  $("#jingle-vol").value = settings.jingles.volume_db;
}

export function init() {
  $("#jingles").addEventListener("click", (e) => {
    const el = e.target.closest(".jingle");
    if (el) send({ type: "jingle_play", index: Number(el.dataset.index) });
  });
  $("#jingle-stop").addEventListener("click", () => send({ type: "jingle_stop_all" }));
  $("#jingle-rescan").addEventListener("click", () => send({ type: "jingle_rescan" }));
  $("#jingle-dir").addEventListener("click", pickDir);
  $("#jingle-vol").addEventListener("change", (e) => {
    invoke("save_jingles", { dirs: null, volumeDb: Number(e.target.value) }).catch(errorOut);
  });
  setJingles(view);
}
