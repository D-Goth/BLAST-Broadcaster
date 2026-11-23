import { $, $$, T, invoke, toast, setResult, confirmClick } from "./core.js";
import { openSettings } from "./settings.js";

// [UI-108]
const I = {
  view: null,
  sel: null,
  draftServer: null,
  tab: "dash",
  status: {},
  history: {},
  rate: {},
  peak: {},
  conf: null,
  confError: null,
  draft: null,
  mount: null,
  logKind: "error",
  busy: false,
  timer: null,
  pillTimer: null,
};

const HISTORY_MS = 10 * 60 * 1000;
const NEW_SERVER = {
  id: "", name: "Icecast local", remote: false, install_dir: null, exe: null, config_path: null,
  host: "localhost", port: 8000, https: false, admin_user: "admin", autostart: false, stop_on_exit: false,
  has_password: false, managed: false,
};
const NEW_REMOTE = { ...NEW_SERVER, name: "Mon VPS", remote: true, host: "radio.exemple.fr", autostart: false };

// [UI-109]
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
const clone = (o) => JSON.parse(JSON.stringify(o));
const nowS = () => Date.now() / 1000;

function bytes(n) {
  if (!n) return "0 o";
  const u = ["o", "Ko", "Mo", "Go", "To"];
  const i = Math.min(u.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  return `${(n / 1024 ** i).toFixed(i ? 1 : 0).replace(".", ",")} ${u[i]}`;
}

function dur(secs) {
  if (secs == null || !isFinite(secs) || secs < 0) return "—";
  const s = Math.floor(secs);
  const d = Math.floor(s / 86400);
  const hh = String(Math.floor((s % 86400) / 3600)).padStart(2, "0");
  const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return d > 0 ? `${d} j ${hh}:${mm}` : `${hh}:${mm}:${ss}`;
}

const getK = (o, k) => k.split(".").reduce((a, p) => (a == null ? a : a[p]), o);
function setK(o, k, v) {
  const ps = k.split(".");
  let a = o;
  for (const p of ps.slice(0, -1)) a = a[p];
  a[ps[ps.length - 1]] = v;
}

const server = () => (I.sel === "new" ? I.draftServer : I.view && I.view.servers.find((s) => s.id === I.sel)) || null;
const isLocal = () => !!server() && !server().remote;
const st = () => (I.sel && I.status[I.sel]) || null;

function stateOf(s) {
  if (!s) return "off";
  if (s.reachable && s.stats && s.stats.sources > 0) return "live";
  if (s.reachable || s.running) return "running";
  return "off";
}

// [UI-110]
function renderServers() {
  const box = $("#ice-servers");
  box.innerHTML = "";
  for (const s of I.view.servers) {
    const status = I.status[s.id];
    const b = document.createElement("button");
    b.className = "ice-server" + (s.id === I.sel ? " active" : "");
    const n = status && status.stats ? status.stats.listeners : null;
    b.innerHTML = `<span class="dot ${stateOf(status) === "running" ? "connecting" : stateOf(status)}"></span><span class="n"></span><span class="c"></span><span class="t"></span>`;
    $(".n", b).textContent = s.name;
    $(".c", b).textContent = n != null ? String(n) : "";
    $(".c", b).title = "auditeurs";
    $(".t", b).textContent = `${s.remote ? "Distant" : "Local"} · ${s.host}:${s.port}`;
    b.addEventListener("click", () => selectServer(s.id));
    box.appendChild(b);
  }
  $("#ice-store").textContent = `Mots de passe admin : ${I.view.secret_store}`;
  renderWelcome();
}

// [UI-129]
function renderWelcome() {
  const box = $("#ice-detected");
  box.innerHTML = "";
  const known = new Set(I.view.servers.map((s) => s.exe).filter(Boolean));
  const list = I.view.detected.filter((d) => !known.has(d.exe));
  if (!list.length) {
    box.innerHTML = `<div class="muted">Aucune autre installation détectée dans les emplacements habituels. Choisis le dossier à la main.</div>`;
  }
  for (const d of list) {
    const row = document.createElement("div");
    row.className = "ice-found";
    row.innerHTML = `<code></code><span class="muted"></span><button class="btn accent small">Ajouter</button>`;
    $("code", row).textContent = d.dir;
    $("span", row).textContent = d.config ? "icecast.xml trouvé" : "sans icecast.xml";
    $("button", row).addEventListener("click", () =>
      newServer({ ...NEW_SERVER, install_dir: d.dir, exe: d.exe, config_path: d.config }),
    );
    box.appendChild(row);
  }
  const empty = !I.sel;
  $("#ice-welcome").classList.toggle("hidden", !empty);
  $("#ice-body").classList.toggle("hidden", empty);
}

function newServer(tpl) {
  I.sel = "new";
  I.draftServer = clone(tpl);
  I.conf = null;
  I.draft = null;
  renderServers();
  renderHead();
  showITab("profile");
}

// [UI-111]
async function selectServer(id) {
  if (I.draft && dirty() && id !== I.sel) toast("warn", "Les modifications non enregistrées de la configuration précédente sont abandonnées.");
  I.sel = id;
  I.conf = null;
  I.draft = null;
  I.confError = null;
  I.mount = null;
  renderServers();
  renderHead();
  const s = server();
  const tab = s && s.remote && ["config", "mounts", "log"].includes(I.tab) ? "dash" : I.tab;
  showITab(tab === "profile" && I.sel !== "new" ? "dash" : tab);
  if (s && !s.remote) await loadConfig();
  await refreshStatus();
}

// [UI-112]
function showITab(tab) {
  const s = server();
  const isNew = I.sel === "new";
  I.tab = tab;
  $$(".ice-tabs .fx-tab").forEach((t) => {
    const off = isNew ? t.dataset.itab !== "profile" : s && s.remote && t.hasAttribute("data-local");
    t.disabled = !!off;
    t.title = off ? (isNew ? "Enregistre d'abord la connexion" : "Disponible pour un serveur local (SSH à venir pour un VPS)") : "";
    t.classList.toggle("active", t.dataset.itab === tab);
  });
  $$(".ice-page").forEach((p) => p.classList.toggle("hidden", p.dataset.ipage !== tab));
  if (tab === "profile") fillProfile();
  if (tab === "config") fillConfig();
  if (tab === "mounts") renderMounts();
  if (tab === "listeners") loadListeners();
  if (tab === "log") loadLog();
  if (tab === "dash") renderDash();
  renderSavebar();
  schedule();
}

// [UI-114]
function renderHead() {
  const s = server();
  if (!s) return;
  const status = st();
  const state = stateOf(status);
  $("#ice-dot").className = "dot " + (state === "live" ? "live" : state === "running" ? "running" : status && status.error && s.remote ? "error" : "");
  $("#ice-name").textContent = s.name || "Nouveau serveur";
  const parts = [`${s.remote ? "Distant" : "Local"} · ${s.https ? "https" : "http"}://${s.host}:${s.port}`];
  if (status && status.server_id) parts.push(status.server_id);
  if (status && status.running && !s.remote) parts.push(status.owned ? "lancé par BLAST" : "processus repris");
  $("#ice-sub").textContent = I.sel === "new" ? "Configure la connexion puis enregistre." : parts.join(" · ");
  const running = !!(status && status.running);
  const external = !!(status && status.reachable && !status.running && !s.remote);
  const local = !s.remote && I.sel !== "new";
  $("[data-power=start]").classList.toggle("hidden", !local || running || external);
  $("[data-power=stop]").classList.toggle("hidden", !local || !running);
  $("[data-power=restart]").classList.toggle("hidden", !local || !running);
  $("#ice-web").classList.toggle("hidden", I.sel === "new" || !(status && status.reachable));
  $$("[data-power]").forEach((b) => (b.disabled = I.busy));
}

// [UI-113]
async function refreshStatus(id = I.sel) {
  if (!id || id === "new") return;
  try {
    const s = await invoke("ice_status", { id });
    I.status[id] = s;
    record(id, s);
  } catch (e) {
    I.status[id] = { error: String(e), running: false, reachable: false };
  }
  renderPill();
  if (!isOpen()) return;
  renderServers();
  if (id === I.sel) {
    renderHead();
    if (I.tab === "dash") renderDash();
  }
}

function record(id, s) {
  const t = Date.now();
  const h = (I.history[id] = (I.history[id] || []).filter((p) => t - p.t < HISTORY_MS));
  if (!s.stats) return;
  h.push({ t, n: s.stats.listeners });
  I.peak[id] = Math.max(I.peak[id] || 0, s.stats.listeners);
  const sent = s.stats.mounts.reduce((a, m) => a + m.total_bytes_sent, 0);
  const r = I.rate[id] && I.rate[id].sent <= sent ? I.rate[id] : { samples: [] };
  r.samples = r.samples.filter((p) => t - p.t < 30000);
  r.samples.push({ t, sent });
  const first = r.samples[0];
  r.sent = sent;
  r.kbps = t - first.t >= 8000 ? ((sent - first.sent) * 8) / ((t - first.t) / 1000) / 1000 : r.kbps ?? null;
  I.rate[id] = r;
}

// [UI-115]
function renderDash() {
  const s = st();
  const banner = $("#ice-banner");
  const srv = server();
  let note = s && s.error ? s.error : "";
  let level = "";
  if (s && !s.running && !s.reachable && srv && !srv.remote) note = "Icecast est arrêté. Clique sur Démarrer.";
  if (s && s.reachable && !s.running && srv && !srv.remote) note = (note ? note + " " : "") + "Icecast répond mais n'a pas été lancé par BLAST (service Windows ou autre fenêtre) : Démarrer et Arrêter sont indisponibles.";
  if (s && s.error && srv && srv.remote && !s.reachable) level = "error";
  banner.textContent = note;
  banner.className = "ice-banner" + (level ? " " + level : "") + (note ? "" : " hidden");
  const stats = s && s.stats;
  const conf = I.conf;
  const set = (id, v) => ($(id).textContent = v);
  set("#it-listeners", stats ? stats.listeners : "—");
  set("#it-listeners-sub", conf ? `limite ${conf.limits.clients}` : "");
  const peak = stats ? Math.max(I.peak[I.sel] || 0, ...stats.mounts.map((m) => m.listener_peak)) : null;
  set("#it-peak", peak ?? "—");
  set("#it-sources", stats ? stats.sources : "—");
  set("#it-sources-sub", conf ? `limite ${conf.limits.sources}` : "");
  set("#it-uptime", stats && stats.started_at ? dur(nowS() - stats.started_at) : "—");
  set("#it-started", stats && stats.started_at ? new Date(stats.started_at * 1000).toLocaleString("fr-FR") : "");
  const r = stats ? I.rate[I.sel] : null;
  set("#it-rate", r && r.kbps != null ? `${Math.round(r.kbps)} kb/s` : "—");
  set("#it-sent", r ? `${bytes(r.sent)} envoyés` : "");
  set("#it-conns", stats ? stats.listener_connections : "—");
  set("#it-conns-sub", stats ? "écoutes au total" : "");
  renderMountTable(stats);
  drawChart();
}

function mountRows(stats) {
  const rows = new Map();
  for (const m of stats ? stats.mounts : []) rows.set(m.mount, { live: m, conf: null });
  for (const m of I.conf ? I.conf.mounts : []) {
    const r = rows.get(m.mount_name) || { live: null, conf: null };
    r.conf = m;
    rows.set(m.mount_name, r);
  }
  return [...rows.entries()].sort((a, b) => a[0].localeCompare(b[0]));
}

function renderMountTable(stats) {
  const tb = $("#ice-mount-table tbody");
  tb.innerHTML = "";
  const rows = mountRows(stats);
  $("#ice-mount-none").classList.toggle("hidden", rows.length > 0);
  for (const [name, { live, conf }] of rows) {
    const tr = document.createElement("tr");
    const on = live && live.connected;
    if (!on) tr.className = "waiting";
    const max = live && live.max_listeners != null ? live.max_listeners : conf && conf.max_listeners ? conf.max_listeners : null;
    const fb = conf && conf.fallback_mount ? ` → secours ${conf.fallback_mount}` : "";
    tr.innerHTML = `
      <td><b></b></td>
      <td><span class="ice-state"><span class="dot ${on ? "live" : ""}"></span>${on ? "Diffuse" : "En attente" + esc(fb)}</span></td>
      <td class="title"></td>
      <td class="mono">${live ? live.listeners : 0}${max ? " / " + max : ""}</td>
      <td class="mono">${live ? live.listener_peak : "—"}</td>
      <td>${live ? esc([live.server_type.replace("audio/", ""), live.bitrate && live.bitrate + " kb/s"].filter(Boolean).join(" · ")) : ""}</td>
      <td class="src"></td>
      <td class="mono">${live && live.stream_start ? dur(nowS() - live.stream_start) : "—"}</td>
      <td class="acts"></td>`;
    $("b", tr).textContent = name;
    $(".title", tr).textContent = live ? live.title || live.server_name || "—" : "—";
    $(".src", tr).textContent = live ? [live.source_ip, live.user_agent].filter(Boolean).join(" · ") : "";
    $(".src", tr).title = $(".src", tr).textContent;
    const acts = $(".acts", tr);
    if (on) acts.appendChild(btn("Actions ▾", (e) => menuMount(e.currentTarget, name, live.title)));
    tb.appendChild(tr);
  }
}

function btn(label, run) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "btn ghost small";
  b.textContent = label;
  b.addEventListener("click", run);
  return b;
}

// [UI-116]
function drawChart() {
  const c = $("#ice-chart");
  const w = c.clientWidth;
  const h = c.clientHeight;
  if (!w || !h) return;
  const dpr = window.devicePixelRatio || 1;
  c.width = Math.round(w * dpr);
  c.height = Math.round(h * dpr);
  const g = c.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const css = getComputedStyle(document.documentElement);
  const accent = css.getPropertyValue("--accent").trim() || "#FF1654";
  const grid = css.getPropertyValue("--border").trim() || "#2A2A2A";
  const muted = css.getPropertyValue("--text-muted").trim() || "#9A9A9A";
  const pts = I.history[I.sel] || [];
  const max = Math.max(4, ...pts.map((p) => p.n));
  $("#ice-chart-max").textContent = pts.length ? `max ${Math.max(...pts.map((p) => p.n))}` : "en attente de données";
  const now = Date.now();
  const x = (t) => w - ((now - t) / HISTORY_MS) * w;
  const y = (n) => h - 6 - (n / max) * (h - 18);
  g.strokeStyle = grid;
  g.lineWidth = 1;
  g.fillStyle = muted;
  g.font = "10px sans-serif";
  for (let i = 0; i <= 2; i++) {
    const v = Math.round((max * i) / 2);
    const yy = Math.round(y(v)) + 0.5;
    g.beginPath();
    g.moveTo(0, yy);
    g.lineTo(w, yy);
    g.stroke();
    g.fillText(String(v), 4, yy - 3);
  }
  if (pts.length < 2) return;
  g.beginPath();
  g.moveTo(x(pts[0].t), y(pts[0].n));
  for (let i = 1; i < pts.length; i++) {
    g.lineTo(x(pts[i].t), y(pts[i - 1].n));
    g.lineTo(x(pts[i].t), y(pts[i].n));
  }
  g.lineTo(w, y(pts[pts.length - 1].n));
  g.strokeStyle = accent;
  g.lineWidth = 2;
  g.stroke();
  g.lineTo(w, h);
  g.lineTo(x(pts[0].t), h);
  g.closePath();
  g.globalAlpha = 0.15;
  g.fillStyle = accent;
  g.fill();
  g.globalAlpha = 1;
}

// [UI-117]
function openMenu(anchor, html, bind) {
  const m = $("#ice-menu");
  m.innerHTML = html;
  m.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  m.style.left = `${Math.max(8, Math.min(window.innerWidth - 296, r.right - 280))}px`;
  m.style.top = `${Math.min(window.innerHeight - m.offsetHeight - 8, r.bottom + 6)}px`;
  bind(m);
}

const closeMenu = () => $("#ice-menu").classList.add("hidden");

function menuMount(anchor, mount, title) {
  openMenu(anchor, `<div class="menu-title">${esc(mount)}</div>
    <button class="btn ghost small" data-a="lis">Voir les auditeurs</button>
    <button class="btn ghost small" data-a="title">Changer le titre affiché…</button>
    <button class="btn ghost small" data-a="move">Déplacer les auditeurs…</button>
    <button class="btn ghost small danger" data-a="kill">Couper la source</button>`, (m) => {
    $("[data-a=lis]", m).addEventListener("click", () => {
      closeMenu();
      $("#ice-lis-mount").dataset.want = mount;
      showITab("listeners");
    });
    $("[data-a=title]", m).addEventListener("click", () => menuTitle(anchor, mount, title));
    $("[data-a=move]", m).addEventListener("click", () => menuMove(anchor, mount));
    const kill = $("[data-a=kill]", m);
    kill.addEventListener("click", () =>
      confirmClick(kill, "Couper la source", "Confirmer : la source sera déconnectée", () => admin({ action: "kill_source", mount }).then(closeMenu)),
    );
  });
}

function menuTitle(anchor, mount, current) {
  openMenu(anchor, `<div class="menu-title">Titre affiché sur ${esc(mount)}</div><input id="im-song"><button class="btn accent small" id="im-go">Mettre à jour</button><div class="muted small-note">La source le remplacera au prochain changement de morceau.</div>`, (m) => {
    const inp = $("#im-song", m);
    inp.value = current || "";
    inp.focus();
    const go = () => admin({ action: "metadata", mount, song: inp.value.trim() }).then(closeMenu);
    $("#im-go", m).addEventListener("click", go);
    inp.addEventListener("keydown", (e) => e.key === "Enter" && go());
  });
}

function menuMove(anchor, mount) {
  const names = mountRows(st() && st().stats).map(([n]) => n).filter((n) => n !== mount);
  if (!names.length) {
    toast("warn", "Aucun autre mountpoint vers lequel déplacer les auditeurs.");
    return;
  }
  openMenu(anchor, `<div class="menu-title">Déplacer les auditeurs de ${esc(mount)}</div><select id="im-dest">${names.map((n) => `<option>${esc(n)}</option>`).join("")}</select><button class="btn accent small" id="im-go">Déplacer</button>`, (m) => {
    $("#im-go", m).addEventListener("click", () => admin({ action: "move_clients", mount, destination: $("#im-dest", m).value }).then(closeMenu));
  });
}

async function admin(action) {
  try {
    const msg = await invoke("ice_admin", { id: I.sel, action });
    toast("info", msg || "Fait.");
  } catch (e) {
    toast("error", String(e));
  }
  setTimeout(() => refreshStatus(), 400);
  if (I.tab === "listeners") loadListeners();
}

// [UI-118]
async function loadConfig() {
  try {
    const v = await invoke("ice_get_config", { id: I.sel });
    applyConfigView(v);
  } catch (e) {
    I.conf = null;
    I.draft = null;
    I.confError = String(e);
  }
  if (I.tab === "config") fillConfig();
  if (I.tab === "mounts") renderMounts();
  renderSavebar();
}

function applyConfigView(v) {
  I.confView = v;
  I.conf = v.config;
  I.draft = clone(v.config);
  I.confError = null;
  for (const w of v.warnings || []) toast("warn", w);
  if (v.message) toast("info", v.message);
}

const dirty = () => !!I.draft && JSON.stringify(I.draft) !== JSON.stringify(I.conf);

function renderSavebar() {
  $("#ice-savebar").classList.toggle("hidden", !(dirty() && ["config", "mounts"].includes(I.tab)));
  const running = !!(st() && st().running);
  $("#ice-save-restart").textContent = running ? "Enregistrer et redémarrer" : "Enregistrer et démarrer";
}

// [UI-119]
function fillConfig() {
  const noconf = !I.draft;
  $("#ice-noconf").classList.toggle("hidden", !noconf);
  $("#ice-config-form").hidden = noconf;
  $("#ice-file").classList.toggle("hidden", noconf);
  if (noconf) {
    const s = server();
    $("#ice-noconf-text").textContent = I.confError || "Aucun fichier de configuration n'est associé à ce serveur.";
    $("#ice-create").classList.toggle("hidden", !s || s.remote || !!(s.config_path && s.managed));
    return;
  }
  const v = I.confView;
  $("#ice-file-path").textContent = v.path;
  $("#ice-file-path").title = v.path;
  const chip = $("#ice-file-state");
  chip.textContent = v.writable ? (v.managed ? "copie gérée par BLAST" : "modifiable") : "lecture seule";
  chip.className = "chip " + (v.writable ? "ok" : "ro");
  $("#ice-file-backups").textContent = v.backups ? `${v.backups} sauvegarde${v.backups > 1 ? "s" : ""}` : "";
  $("#ice-adopt").classList.toggle("hidden", v.writable);
  const form = $("#ice-config-form");
  for (const el of $$("[data-k]", form)) {
    const val = getK(I.draft, el.dataset.k);
    if (el.type === "checkbox") el.checked = !!val;
    else el.value = val ?? "";
    el.disabled = !v.writable;
  }
  $$("[data-gen]", form).forEach((b) => (b.disabled = !v.writable));
  $("#ice-socket-add").disabled = !v.writable;
  renderSockets();
}

function readInput(el) {
  if (el.type === "checkbox") return el.checked;
  if (el.type === "number" || el.hasAttribute("data-num")) return Math.max(0, Math.floor(Number(el.value) || 0));
  return el.value;
}

// [UI-120]
function renderSockets() {
  const box = $("#ice-sockets");
  box.innerHTML = "";
  const ro = I.confView && !I.confView.writable;
  I.draft.sockets.forEach((s, i) => {
    const row = document.createElement("div");
    row.className = "ice-row";
    row.innerHTML = `
      <label>Port<input type="number" min="1" max="65535" data-s="port"></label>
      <label>Adresse d'écoute<input data-s="bind_address" placeholder="toutes (0.0.0.0)"></label>
      <label title="Pour les vieux logiciels qui ne parlent que le protocole Shoutcast : ils se connectent au port suivant et arrivent sur ce point de montage">Compatibilité Shoutcast (facultatif)<input data-s="shoutcast_mount" placeholder="laisser vide en général"></label>
      <label class="check"><input type="checkbox" data-s="ssl"> HTTPS</label>
      <button type="button" class="btn ghost small danger">Retirer</button>`;
    for (const el of $$("[data-s]", row)) {
      const k = el.dataset.s;
      if (el.type === "checkbox") el.checked = !!s[k];
      else el.value = s[k] ?? "";
      el.disabled = ro;
      el.addEventListener("input", () => {
        s[k] = readInput(el);
        renderSavebar();
      });
    }
    const del = $("button", row);
    del.disabled = ro || I.draft.sockets.length < 2;
    del.title = I.draft.sockets.length < 2 ? "Il faut au moins un port d'écoute" : "";
    del.addEventListener("click", () => {
      I.draft.sockets.splice(i, 1);
      renderSockets();
      renderSavebar();
    });
    box.appendChild(row);
  });
}

// [UI-121]
function renderMounts() {
  const list = $("#ice-mount-list");
  list.innerHTML = "";
  const has = !!I.draft;
  $("#ice-mount-add").disabled = !has || (I.confView && !I.confView.writable);
  if (!has) {
    $("#ice-mount-empty").textContent = I.confError || "Associe d'abord un fichier de configuration (onglet CONFIGURATION).";
    $("#ice-mount-empty").classList.remove("hidden");
    $("#ice-mount-body").classList.add("hidden");
    return;
  }
  const live = new Map(((st() && st().stats && st().stats.mounts) || []).map((m) => [m.mount, m]));
  I.draft.mounts.forEach((m, i) => {
    const el = document.createElement("div");
    el.className = "out-item" + (i === I.mount ? " active" : "");
    const l = live.get(m.mount_name);
    el.innerHTML = `<div class="n"><span class="dot ${l && l.connected ? "live" : ""}"></span><span></span></div><div class="t"></div>`;
    $(".n span:last-child", el).textContent = m.mount_name || "(sans nom)";
    $(".t", el).textContent = [m.max_listeners ? `${m.max_listeners} auditeurs max` : "sans limite", m.fallback_mount && `secours ${m.fallback_mount}`, m.password && "mot de passe propre"].filter(Boolean).join(" · ");
    el.addEventListener("click", () => {
      I.mount = i;
      renderMounts();
    });
    list.appendChild(el);
  });
  if (I.mount != null && I.mount >= I.draft.mounts.length) I.mount = null;
  if (I.mount == null && I.draft.mounts.length) I.mount = 0;
  const m = I.mount != null ? I.draft.mounts[I.mount] : null;
  $("#ice-mount-empty").textContent = "Sans mountpoint déclaré, Icecast accepte n'importe quel point de montage avec le mot de passe des sources. En déclarer un permet de lui donner un mot de passe propre, une limite d'auditeurs et un secours.";
  $("#ice-mount-empty").classList.toggle("hidden", !!m);
  $("#ice-mount-body").classList.toggle("hidden", !m);
  if (m) {
    list.children[I.mount] && list.children[I.mount].classList.add("active");
    fillMount(m);
  }
}

function fillMount(m) {
  const ro = I.confView && !I.confView.writable;
  for (const el of $$("[data-m]", $("#ice-mount-form"))) {
    const k = el.dataset.m;
    if (k === "public") el.value = m.public == null ? "" : m.public ? "1" : "0";
    else if (el.type === "checkbox") el.checked = !!m[k];
    else el.value = m[k] ?? "";
    el.disabled = ro;
  }
  $$("#ice-mount-form [data-gen]").forEach((b) => (b.disabled = ro));
  $("#ice-mount-delete").disabled = ro;
  $("#ice-mount-names").innerHTML = I.draft.mounts.filter((x) => x !== m).map((x) => `<option value="${esc(x.mount_name)}">`).join("");
  setResult("#ice-mount-result", "", "");
}

function mountInput(el) {
  const m = I.mount != null && I.draft ? I.draft.mounts[I.mount] : null;
  if (!m) return;
  const k = el.dataset.m;
  m[k] = k === "public" ? (el.value === "" ? null : el.value === "1") : readInput(el);
  if (k === "mount_name" || k === "max_listeners" || k === "fallback_mount" || k === "password") {
    const item = $("#ice-mount-list").children[I.mount];
    if (item) {
      $(".n span:last-child", item).textContent = m.mount_name || "(sans nom)";
    }
  }
  renderSavebar();
}

// [UI-122]
async function saveConfig(restart) {
  if (!I.draft) return;
  setResult("#ice-save-result", "busy", restart ? "Enregistrement et redémarrage…" : "Enregistrement…");
  try {
    const v = await invoke("ice_save_config", { id: I.sel, config: I.draft, restart });
    applyConfigView(v);
    setResult("#ice-save-result", "", "");
    I.view = await invoke("ice_list");
    renderServers();
    showITab(I.tab);
    setTimeout(() => refreshStatus(), 800);
  } catch (e) {
    setResult("#ice-save-result", "err", String(e));
  }
}

// [UI-123]
async function loadListeners() {
  if (I.tab !== "listeners" || !I.sel || I.sel === "new") return;
  const sel = $("#ice-lis-mount");
  const mounts = ((st() && st().stats && st().stats.mounts) || []).filter((m) => m.connected).map((m) => m.mount);
  const want = sel.dataset.want || sel.value;
  sel.dataset.want = "";
  sel.innerHTML = mounts.map((m) => `<option>${esc(m)}</option>`).join("");
  if (mounts.includes(want)) sel.value = want;
  const tb = $("#ice-lis-table tbody");
  if (!mounts.length) {
    tb.innerHTML = "";
    $("#ice-lis-none").textContent = "Aucun mountpoint ne diffuse en ce moment.";
    $("#ice-lis-none").classList.remove("hidden");
    $("#ice-lis-info").textContent = "";
    return;
  }
  try {
    const list = await invoke("ice_listeners", { id: I.sel, mount: sel.value });
    tb.innerHTML = "";
    $("#ice-lis-none").textContent = "Personne à l'écoute sur ce mountpoint.";
    $("#ice-lis-none").classList.toggle("hidden", list.length > 0);
    $("#ice-lis-info").textContent = `${list.length} auditeur${list.length > 1 ? "s" : ""}`;
    for (const l of list.sort((a, b) => b.connected_secs - a.connected_secs)) {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td class="ip"></td><td class="ua"></td><td class="mono">${dur(l.connected_secs)}</td><td class="mono">${l.id}</td><td class="acts"></td>`;
      $(".ip", tr).textContent = l.ip;
      $(".ua", tr).textContent = l.user_agent || "—";
      $(".ua", tr).title = l.user_agent;
      const kick = btn("Déconnecter", () => {});
      kick.classList.add("danger");
      kick.onclick = () => confirmClick(kick, "Déconnecter", "Confirmer", () => admin({ action: "kill_client", mount: sel.value, id: l.id }));
      $(".acts", tr).appendChild(kick);
      tb.appendChild(tr);
    }
  } catch (e) {
    tb.innerHTML = "";
    $("#ice-lis-none").textContent = String(e);
    $("#ice-lis-none").classList.remove("hidden");
  }
}

// [UI-124]
async function loadLog() {
  if (I.tab !== "log" || !I.sel || I.sel === "new") return;
  const pre = $("#ice-log");
  const atEnd = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 30;
  try {
    const text = await invoke("ice_log", { id: I.sel, which: I.logKind, lines: 500 });
    pre.innerHTML = text
      .split("\n")
      .map((l) => {
        const cls = / EROR /.test(l) ? "e" : / WARN /.test(l) ? "w" : "";
        return cls ? `<span class="${cls}">${esc(l)}</span>` : esc(l);
      })
      .join("\n");
  } catch (e) {
    pre.textContent = String(e);
  }
  if (atEnd) pre.scrollTop = pre.scrollHeight;
}

// [UI-125]
function fillProfile() {
  const s = server();
  if (!s) return;
  const f = $("#ice-profile-form").elements;
  f.name.value = s.name;
  f.remote.value = String(!!s.remote);
  f.install_dir.value = s.install_dir || "";
  f.config_path.value = s.config_path || "";
  f.host.value = s.host;
  f.port.value = s.port;
  f.admin_user.value = s.admin_user;
  f.https.checked = !!s.https;
  f.autostart.checked = !!s.autostart;
  f.stop_on_exit.checked = !!s.stop_on_exit;
  f.password.value = "";
  f.password.placeholder = s.has_password ? "•••••• enregistré (laisser vide pour garder)" : s.remote ? "mot de passe admin du serveur" : "lu dans icecast.xml si vide";
  $("#ice-delete").classList.toggle("hidden", I.sel === "new");
  setResult("#ice-profile-result", "", "");
  applyWhere();
}

function applyWhere() {
  const f = $("#ice-profile-form").elements;
  const remote = f.remote.value === "true";
  $$("#ice-profile-form [data-where=local]").forEach((el) => el.classList.toggle("hidden", remote));
  const s = server();
  $("#ice-exe-info").textContent = remote ? "" : s && s.exe ? `Exécutable : ${s.exe}` : "L'exécutable sera cherché dans le dossier (bin\\icecast.exe sous Windows).";
  const https = f.https.checked;
  const host = f.host.value.trim();
  const lan = ["localhost", "127.0.0.1", "::1"].includes(host);
  $("#ice-pw-note").textContent = remote && !https && !lan
    ? "⚠ En HTTP, le mot de passe admin circule en clair sur Internet à chaque actualisation. Mets Icecast derrière HTTPS (proxy Nginx/Caddy) ou passe par un tunnel SSH vers localhost."
    : `Le mot de passe est rangé dans ${I.view ? I.view.secret_store : "le trousseau du système"}, jamais dans les fichiers de BLAST.`;
}

async function saveProfile() {
  const s = server();
  const f = $("#ice-profile-form").elements;
  const remote = f.remote.value === "true";
  const profile = {
    id: I.sel === "new" ? "" : s.id,
    name: f.name.value.trim(),
    remote,
    install_dir: remote ? null : f.install_dir.value.trim() || null,
    exe: remote ? null : s.install_dir === f.install_dir.value.trim() ? s.exe : null,
    config_path: remote ? null : f.config_path.value.trim() || null,
    host: f.host.value.trim(),
    port: Number(f.port.value) || 0,
    https: f.https.checked,
    admin_user: f.admin_user.value.trim(),
    autostart: f.autostart.checked,
    stop_on_exit: f.stop_on_exit.checked,
  };
  setResult("#ice-profile-result", "busy", "Enregistrement…");
  try {
    const before = new Set(I.view.servers.map((x) => x.id));
    I.view = await invoke("ice_save_server", { server: profile, password: f.password.value || null });
    if (I.view.warning) toast("warn", I.view.warning);
    const id = profile.id || (I.view.servers.find((x) => !before.has(x.id)) || {}).id;
    setResult("#ice-profile-result", "ok", "Enregistré.");
    I.tab = "dash";
    await selectServer(id);
  } catch (e) {
    setResult("#ice-profile-result", "err", String(e));
  }
}

async function pickDir() {
  const dir = await T.dialog.open({ directory: true, multiple: false, title: "Dossier d'installation d'Icecast" });
  if (!dir) return;
  const f = $("#ice-profile-form").elements;
  f.install_dir.value = dir;
  const found = await invoke("ice_detect", { dir }).catch(() => []);
  if (found.length) {
    if (!f.config_path.value && found[0].config) f.config_path.value = found[0].config;
    $("#ice-exe-info").textContent = `Exécutable : ${found[0].exe}`;
  } else {
    $("#ice-exe-info").textContent = "⚠ Aucun icecast.exe dans ce dossier (ni dans son sous-dossier bin).";
  }
}

async function pickXml() {
  const file = await T.dialog.open({ multiple: false, directory: false, title: "Configuration Icecast", filters: [{ name: "Icecast", extensions: ["xml"] }] });
  if (file) $("#ice-profile-form").elements.config_path.value = file;
}

// [UI-128]
async function power(action) {
  I.busy = true;
  renderHead();
  try {
    toast("info", await invoke("ice_power", { id: I.sel, action }));
  } catch (e) {
    toast("error", String(e));
    if (I.tab !== "log") {
      I.logKind = "console";
      $$("[data-ilog]").forEach((b) => b.classList.toggle("active", b.dataset.ilog === "console"));
    }
  }
  I.busy = false;
  await refreshStatus();
  renderSavebar();
}

// [UI-126]
function renderPill() {
  const pill = $("#ice-pill");
  const servers = (I.view && I.view.servers) || [];
  const states = servers.map((s) => I.status[s.id]);
  const live = states.filter((s) => stateOf(s) === "live").length;
  const running = states.filter((s) => stateOf(s) !== "off").length;
  const err = servers.some((s) => s.remote && I.status[s.id] && !I.status[s.id].reachable);
  pill.className = "obs-pill ice-pill" + (live ? " live" : running ? " running" : err ? " error" : "");
  const total = states.reduce((a, s) => a + (s && s.stats ? s.stats.listeners : 0), 0);
  const count = $("#ice-pill-count");
  count.textContent = String(total);
  count.classList.toggle("hidden", !running);
  pill.title = servers.length
    ? servers.map((s) => `${s.name} : ${stateOf(I.status[s.id]) === "off" ? "arrêté ou injoignable" : (I.status[s.id].stats ? I.status[s.id].stats.listeners : "?") + " auditeur(s)"}`).join("\n")
    : "Serveurs Icecast : lancer, configurer, surveiller";
}

async function pollAll() {
  if (!I.view) return;
  for (const s of I.view.servers) {
    if (isOpen() && s.id === I.sel) continue;
    await refreshStatus(s.id);
  }
}

function schedule() {
  clearInterval(I.timer);
  if (!isOpen() || !I.sel || I.sel === "new") return;
  const every = I.tab === "dash" ? 2000 : I.tab === "log" ? 3000 : 5000;
  I.timer = setInterval(() => {
    refreshStatus();
    if (I.tab === "listeners") loadListeners();
    if (I.tab === "log" && $("#ice-log-auto").checked) loadLog();
  }, every);
}

// [UI-130]
async function quickLink() {
  try {
    const msg = await invoke("ice_link_output", { id: I.sel, mount: "/live" });
    toast("info", msg, [{ label: "Voir la sortie", run: () => { closeIcecast(); openSettings("cast"); } }]);
  } catch (e) {
    toast("error", String(e));
  }
}

// [UI-127]
export const isOpen = () => !$("#icecast").classList.contains("hidden");

export async function openIcecast() {
  try {
    I.view = await invoke("ice_list");
  } catch (e) {
    toast("error", String(e));
    return;
  }
  $("#icecast").classList.remove("hidden");
  if (I.sel && I.sel !== "new" && !I.view.servers.some((s) => s.id === I.sel)) I.sel = null;
  if (!I.sel && I.view.servers.length) I.sel = I.view.servers[0].id;
  renderServers();
  if (I.sel) await selectServer(I.sel);
  pollAll();
}

function closeIcecast() {
  $("#icecast").classList.add("hidden");
  closeMenu();
  clearInterval(I.timer);
}

export function init() {
  $("#ice-pill").addEventListener("click", openIcecast);
  $("#ice-close").addEventListener("click", closeIcecast);
  $("#icecast").addEventListener("pointerdown", (e) => {
    if (e.target.id === "icecast") closeIcecast();
  });
  document.addEventListener("pointerdown", (e) => {
    if (!e.target.closest("#ice-menu") && !e.target.closest(".acts")) closeMenu();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape" || !isOpen()) return;
    if (!$("#ice-menu").classList.contains("hidden")) closeMenu();
    else closeIcecast();
  });
  $("#ice-add").addEventListener("click", () => {
    I.sel = null;
    renderServers();
  });
  $("#ice-add-local").addEventListener("click", () => newServer(NEW_SERVER));
  $("#ice-add-remote").addEventListener("click", () => newServer(NEW_REMOTE));
  $$(".ice-tabs .fx-tab").forEach((t) => t.addEventListener("click", () => !t.disabled && showITab(t.dataset.itab)));
  $$("[data-power]").forEach((b) => b.addEventListener("click", () => power(b.dataset.power)));
  $("#ice-web").addEventListener("click", () => invoke("ice_open_web", { id: I.sel }).catch((e) => toast("error", String(e))));

  $("#ice-config-form").addEventListener("input", (e) => {
    const el = e.target;
    if (!el.dataset.k || !I.draft) return;
    setK(I.draft, el.dataset.k, readInput(el));
    renderSavebar();
  });
  $("#ice-config-form").addEventListener("submit", (e) => e.preventDefault());
  $("#ice-socket-add").addEventListener("click", () => {
    const used = new Set(I.draft.sockets.map((s) => s.port));
    let port = 8000;
    while (used.has(port)) port += 2;
    I.draft.sockets.push({ key: null, port, bind_address: "", shoutcast_mount: "", ssl: false });
    renderSockets();
    renderSavebar();
  });
  $("#ice-adopt").addEventListener("click", async () => {
    try {
      applyConfigView(await invoke("ice_adopt_config", { id: I.sel }));
      I.view = await invoke("ice_list");
      fillConfig();
    } catch (e) {
      toast("error", String(e));
    }
  });
  $("#ice-create").addEventListener("click", async () => {
    try {
      applyConfigView(await invoke("ice_new_config", { id: I.sel }));
      I.view = await invoke("ice_list");
      renderServers();
      fillConfig();
    } catch (e) {
      toast("error", String(e));
    }
  });
  $("#ice-goto-profile").addEventListener("click", () => showITab("profile"));

  for (const form of ["#ice-config-form", "#ice-mount-form"]) {
    $(form).addEventListener("click", (e) => {
      const eye = e.target.closest("[data-eye]");
      const gen = e.target.closest("[data-gen]");
      const input = (eye || gen) && (eye || gen).parentElement.querySelector("input");
      if (eye) input.type = input.type === "password" ? "text" : "password";
      if (gen) {
        const a = "abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        const r = crypto.getRandomValues(new Uint32Array(16));
        input.value = Array.from(r, (n) => a[n % a.length]).join("");
        input.type = "text";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      }
    });
  }

  $("#ice-mount-form").addEventListener("input", (e) => e.target.dataset.m && mountInput(e.target));
  $("#ice-mount-form").addEventListener("change", (e) => e.target.dataset.m && mountInput(e.target));
  $("#ice-mount-form").addEventListener("focusout", (e) => e.target.dataset.m && renderMounts());
  $("#ice-mount-add").addEventListener("click", () => {
    const names = new Set(I.draft.mounts.map((m) => m.mount_name));
    let name = names.has("/live") ? "/autodj" : "/live";
    for (let i = 2; names.has(name); i++) name = `/live${i}`;
    I.draft.mounts.push({ key: null, mount_name: name, username: "", password: "", max_listeners: 0, fallback_mount: "", fallback_override: false, public: null, stream_name: "", stream_description: "", stream_url: "", genre: "", bitrate: "", hidden: false });
    I.mount = I.draft.mounts.length - 1;
    renderMounts();
    renderSavebar();
  });
  $("#ice-mount-delete").addEventListener("click", () => {
    if (I.mount == null) return;
    I.draft.mounts.splice(I.mount, 1);
    I.mount = null;
    renderMounts();
    renderSavebar();
  });
  $("#ice-quick-link").addEventListener("click", quickLink);
  $("#ice-mount-link").addEventListener("click", async () => {
    const m = I.mount != null ? I.draft.mounts[I.mount] : null;
    if (!m) return;
    if (dirty()) {
      setResult("#ice-mount-result", "err", "Enregistre d'abord la configuration.");
      return;
    }
    try {
      const msg = await invoke("ice_link_output", { id: I.sel, mount: m.mount_name });
      setResult("#ice-mount-result", "ok", "Sortie ajoutée.");
      toast("info", msg, [{ label: "Voir la sortie", run: () => { closeIcecast(); openSettings("cast"); } }]);
    } catch (e) {
      setResult("#ice-mount-result", "err", String(e));
    }
  });

  $("#ice-save").addEventListener("click", () => saveConfig(false));
  $("#ice-save-restart").addEventListener("click", () => saveConfig(true));
  $("#ice-revert").addEventListener("click", () => {
    I.draft = clone(I.conf);
    showITab(I.tab);
  });

  $("#ice-lis-mount").addEventListener("change", loadListeners);
  $("#ice-lis-refresh").addEventListener("click", loadListeners);
  $$("[data-ilog]").forEach((b) =>
    b.addEventListener("click", () => {
      I.logKind = b.dataset.ilog;
      $$("[data-ilog]").forEach((x) => x.classList.toggle("active", x === b));
      $("#ice-log").scrollTop = 1e9;
      loadLog();
    }),
  );
  $("#ice-log-refresh").addEventListener("click", loadLog);

  const pf = $("#ice-profile-form");
  pf.addEventListener("submit", (e) => {
    e.preventDefault();
    saveProfile();
  });
  pf.elements.remote.addEventListener("change", () => {
    const f = pf.elements;
    if (f.remote.value === "true" && f.host.value === "localhost") f.host.value = "";
    if (f.remote.value === "false" && !f.host.value) f.host.value = "localhost";
    applyWhere();
  });
  pf.elements.https.addEventListener("change", applyWhere);
  pf.elements.host.addEventListener("input", applyWhere);
  $("#ice-pick-dir").addEventListener("click", pickDir);
  $("#ice-pick-xml").addEventListener("click", pickXml);
  const del = $("#ice-delete");
  del.addEventListener("click", () =>
    confirmClick(del, "Retirer ce serveur", "Confirmer (le fichier icecast.xml n'est pas supprimé)", async () => {
      try {
        I.view = await invoke("ice_delete_server", { id: I.sel });
        delete I.status[I.sel];
        I.sel = I.view.servers.length ? I.view.servers[0].id : null;
        renderServers();
        if (I.sel) selectServer(I.sel);
        renderPill();
      } catch (e) {
        setResult("#ice-profile-result", "err", String(e));
      }
    }),
  );

  window.addEventListener("resize", () => isOpen() && I.tab === "dash" && drawChart());
  invoke("ice_list")
    .then((v) => {
      I.view = v;
      renderPill();
      pollAll();
    })
    .catch(() => {});
  I.pillTimer = setInterval(() => !isOpen() && pollAll(), 10000);
}
