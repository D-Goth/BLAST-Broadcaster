import { $, invoke, toast, setResult } from "./core.js";

const UPGRADE = {
  linux: "git pull && ./scripts/install-linux.sh",
  windows: "git pull puis scripts\\install-windows.bat",
};

let info = null;

const openLink = (url) => invoke("open_link", { url }).catch((e) => toast("error", String(e)));

function describe(u) {
  if (!u.latest) return `Aucune version publiée pour l'instant (installée : ${u.current}).`;
  if (u.newer) return `Nouvelle version disponible : ${u.latest} (installée : ${u.current}).`;
  return `BLAST est à jour (${u.current}).`;
}

function announce(u) {
  const hint = UPGRADE[info && info.platform] || UPGRADE.linux;
  toast("update", `BLAST ${u.latest} est disponible — dans le dossier du projet : ${hint}`, [
    { label: "Voir la version", run: () => openLink(u.url) },
    { label: "Ignorer cette version", run: () => invoke("save_updates", { checkOnStart: null, skip: u.latest }) },
  ]);
}

export async function checkOnStart() {
  try {
    info = await invoke("app_info");
    if (!info.updates.check_on_start) return;
    const u = await invoke("check_update", { automatic: true });
    if (u && u.newer) announce(u);
  } catch (_) {}
}

async function checkNow() {
  setResult("#about-result", "busy", "Interrogation de GitHub…");
  try {
    const u = await invoke("check_update", { automatic: false });
    setResult("#about-result", u.newer ? "ok" : "", describe(u));
    $("#about-release").classList.toggle("hidden", !u.newer);
    $("#about-release").onclick = () => openLink(u.url);
  } catch (e) {
    setResult("#about-result", "err", String(e));
  }
}

export async function renderAbout() {
  try {
    info = await invoke("app_info");
  } catch (e) {
    toast("error", String(e));
    return;
  }
  $("#about-version").textContent = info.version;
  $("#about-platform").textContent = info.platform;
  $("#about-upgrade").textContent = UPGRADE[info.platform] || UPGRADE.linux;
  $("#about-log").textContent = info.log_path || "—";
  $("#about-config").textContent = info.config_dir || "—";
  $("#about-auto").checked = !!info.updates.check_on_start;
}

export function init() {
  $("#about-check").addEventListener("click", checkNow);
  $("#about-repo").addEventListener("click", () => info && openLink(info.repository));
  $("#about-auto").addEventListener("change", (e) => {
    invoke("save_updates", { checkOnStart: e.target.checked, skip: null }).catch((err) => toast("error", String(err)));
  });
  $("#about-open-log").addEventListener("click", () => invoke("open_folder", { which: "log" }).catch((e) => toast("error", String(e))));
  $("#about-open-config").addEventListener("click", () => invoke("open_folder", { which: "config" }).catch((e) => toast("error", String(e))));
}
