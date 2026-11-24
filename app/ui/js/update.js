import { $, invoke, listen, toast, setResult } from "./core.js";

// [UI-74]
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

// [UI-155]
let installing = false;
let progressToast = null;

function showProgress(text) {
  if (!progressToast || !progressToast.isConnected) progressToast = toast("update", text);
  progressToast.firstChild.textContent = text;
}

// [UI-156]
async function installNow() {
  if (installing) return;
  installing = true;
  showProgress("Téléchargement de la mise à jour…");
  setResult("#about-result", "busy", "Téléchargement de la mise à jour…");
  try {
    await invoke("install_update");
  } catch (e) {
    if (progressToast) progressToast.remove();
    toast("error", String(e));
    setResult("#about-result", "err", String(e));
  } finally {
    installing = false;
  }
}

// [UI-75]
function announce(u) {
  if (u.installable) {
    toast("update", `BLAST ${u.latest} est disponible.`, [
      { label: "Installer et redémarrer", run: installNow },
      { label: "Voir la version", run: () => openLink(u.url) },
      { label: "Ignorer cette version", run: () => invoke("save_updates", { checkOnStart: null, skip: u.latest }) },
    ]);
    return;
  }
  const hint = UPGRADE[info && info.platform] || UPGRADE.linux;
  toast("update", `BLAST ${u.latest} est disponible — dans le dossier du projet : ${hint}`, [
    { label: "Voir la version", run: () => openLink(u.url) },
    { label: "Ignorer cette version", run: () => invoke("save_updates", { checkOnStart: null, skip: u.latest }) },
  ]);
}

// [UI-76]
export async function checkOnStart() {
  try {
    info = await invoke("app_info");
    if (!info.updates.check_on_start) return;
    const u = await invoke("check_update", { automatic: true });
    if (u && u.newer) announce(u);
  } catch (_) {}
}

// [UI-77]
async function checkNow() {
  setResult("#about-result", "busy", "Interrogation de GitHub…");
  try {
    const u = await invoke("check_update", { automatic: false });
    setResult("#about-result", u.newer ? "ok" : "", describe(u));
    $("#about-release").classList.toggle("hidden", !u.newer);
    $("#about-release").onclick = () => openLink(u.url);
    $("#about-install").classList.toggle("hidden", !u.installable);
  } catch (e) {
    setResult("#about-result", "err", String(e));
  }
}

// [UI-78]
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
  $("#about-upgrade").classList.toggle("hidden", !!info.installer);
  $("#about-mode").textContent = info.installer
    ? `Version installée par paquet (${info.installer}) : la mise à jour se télécharge et s'installe depuis BLAST.`
    : "Version compilée depuis les sources : pour la mettre à jour, dans le dossier du projet :";
  $("#about-log").textContent = info.log_path || "—";
  $("#about-config").textContent = info.config_dir || "—";
  $("#about-auto").checked = !!info.updates.check_on_start;
}

export function init() {
  $("#about-check").addEventListener("click", checkNow);
  $("#about-install").addEventListener("click", installNow);
  listen("blast://update-progress", (e) => {
    const [done, total] = e.payload;
    const mb = (n) => (n / 1048576).toFixed(1).replace(".", ",");
    const text = total
      ? `Téléchargement de la mise à jour : ${Math.round((100 * done) / total)} % (${mb(done)} / ${mb(total)} Mo)`
      : `Téléchargement de la mise à jour : ${mb(done)} Mo`;
    showProgress(total && done >= total ? "Installation… BLAST va redémarrer." : text);
    setResult("#about-result", "busy", text);
  });
  $("#about-repo").addEventListener("click", () => info && openLink(info.repository));
  $("#about-auto").addEventListener("change", (e) => {
    invoke("save_updates", { checkOnStart: e.target.checked, skip: null }).catch((err) => toast("error", String(err)));
  });
  $("#about-open-log").addEventListener("click", () => invoke("open_folder", { which: "log" }).catch((e) => toast("error", String(e))));
  $("#about-open-config").addEventListener("click", () => invoke("open_folder", { which: "config" }).catch((e) => toast("error", String(e))));
}
