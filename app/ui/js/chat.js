import { $, send, toast } from "./core.js";
import { openSettings } from "./settings.js";

// [UI-133]
const KEEP = 300;
const BADGES = { broadcaster: ["B", "Diffuseur"], moderator: ["M", "Modérateur"], vip: ["V", "VIP"], subscriber: ["S", "Abonné"] };
const PALETTE = ["#FF6B6B", "#4DABF7", "#69DB7C", "#FFD43B", "#DA77F2", "#FF922B", "#3BC9DB", "#F783AC"];

const PLATFORMS = { twitch: ["TW", "#9146FF"], youtube: ["YT", "#FF0000"], tiktok: ["TT", "#25F4EE"], kick: ["K", "#53FC18"], facebook: ["FB", "#1877F2"] };
const IDLE = { enabled: false, running: false, connected: false, writable: false, detail: "" };

let view = { channel: "", active: false, twitch: { ...IDLE }, ssn: { ...IDLE }, requests: [] };
let items = [];
let unread = 0;
let visible = false;

const chatShown = () => visible;

// [UI-134]
function colorOf(m) {
  if (m.color) return m.color;
  let h = 0;
  for (const c of m.login || m.author) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return PALETTE[h % PALETTE.length];
}

function hhmm(ms) {
  return new Date(ms || Date.now()).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
}

// [UI-135]
function row(e) {
  const el = document.createElement("div");
  if (e.kind === "notice") {
    el.className = "chat-row notice";
    el.textContent = e.text;
    return el;
  }
  el.className = "chat-row" + (e.own ? " own" : "") + (e.action ? " action" : "");
  el.dataset.id = e.id;
  const time = document.createElement("span");
  time.className = "chat-time";
  time.textContent = hhmm(e.at_ms);
  el.appendChild(time);
  if (view.ssn.enabled && e.source) {
    const p = PLATFORMS[e.source] || [e.source.slice(0, 2).toUpperCase(), "#666"];
    const chip = document.createElement("span");
    chip.className = "chat-platform";
    chip.textContent = p[0];
    chip.title = e.source;
    chip.style.background = p[1];
    el.appendChild(chip);
  }
  for (const b of e.badges || []) {
    const def = BADGES[b];
    if (!def) continue;
    const s = document.createElement("span");
    s.className = `chat-badge ${b}`;
    s.textContent = def[0];
    s.title = def[1];
    el.appendChild(s);
  }
  const who = document.createElement("b");
  who.className = "chat-author";
  who.style.color = colorOf(e);
  who.textContent = e.author;
  el.append(who, document.createTextNode(e.action ? " " : " : "));
  const text = document.createElement("span");
  text.className = "chat-text" + (/^\s*!\w/.test(e.text) ? " cmd" : "");
  text.textContent = e.text;
  el.appendChild(text);
  if (e.donation) {
    const d = document.createElement("span");
    d.className = "chat-donation";
    d.textContent = e.donation;
    el.appendChild(d);
    el.classList.add("donation");
  }
  return el;
}

function renderLog() {
  const log = $("#chat-log");
  log.replaceChildren(...items.map(row));
  if (!items.length) log.innerHTML = `<div class="chat-empty muted">${view.active ? "En attente de messages…" : "Connecte-toi au chat de ta chaîne avec le bouton Connecter."}</div>`;
  log.scrollTop = log.scrollHeight;
}

function append(e) {
  items.push(e);
  if (items.length > KEEP) items.splice(0, items.length - KEEP);
  const log = $("#chat-log");
  const stick = log.scrollHeight - log.scrollTop - log.clientHeight < 40;
  const empty = $(".chat-empty", log);
  if (empty) empty.remove();
  log.appendChild(row(e));
  while (log.children.length > KEEP) log.firstChild.remove();
  if (stick) log.scrollTop = log.scrollHeight;
  if (!chatShown() && e.kind === "message" && !e.own) {
    unread += 1;
    renderUnread();
  }
}

function renderUnread() {
  const b = $("#chat-unread");
  b.textContent = unread > 99 ? "99+" : String(unread);
  b.classList.toggle("hidden", unread === 0);
}

// [UI-136]
function renderRequests() {
  const list = $("#chat-req-list");
  list.innerHTML = "";
  $("#chat-req-count").textContent = view.requests.length ? `${view.requests.length}` : "";
  if (!view.requests.length) {
    list.innerHTML = `<div class="muted chat-req-empty">Aucune demande. Les viewers écrivent <code>!request Artiste - Titre</code>.</div>`;
    return;
  }
  for (const r of view.requests) {
    const card = document.createElement("div");
    card.className = "chat-req" + (r.path ? "" : " missing");
    const head = document.createElement("div");
    head.className = "chat-req-user";
    head.textContent = `${r.user}${view.ssn.enabled && r.platform ? ` (${r.platform})` : ""} · ${hhmm(r.at_ms)}`;
    const song = document.createElement("div");
    song.className = "chat-req-song";
    song.textContent = r.path ? (r.artist ? `${r.artist} - ${r.title}` : r.title) : `Introuvable : « ${r.query} »`;
    const acts = document.createElement("div");
    acts.className = "chat-req-acts";
    const ok = document.createElement("button");
    ok.className = "btn small accent";
    ok.textContent = "Jouer ensuite";
    ok.disabled = !r.path;
    ok.title = "Ajoute le titre en tête de playlist";
    ok.addEventListener("click", () => send({ type: "request_accept", id: r.id }));
    const no = document.createElement("button");
    no.className = "btn small ghost";
    no.textContent = r.path ? "Refuser" : "Retirer";
    no.addEventListener("click", () => send({ type: "request_reject", id: r.id }));
    acts.append(ok, no);
    card.append(head, song, acts);
    list.appendChild(card);
  }
}

// [UI-137]
function sourceLabel(name, v) {
  if (!v.running) return null;
  if (v.connected) return name + (v.writable ? "" : " · lecture seule");
  return `${name} · ${v.detail || "connexion…"}`;
}

function renderStatus() {
  const tw = view.twitch;
  const ssn = view.ssn;
  const any = tw.connected || ssn.connected;
  const pending = (tw.running && !tw.connected) || (ssn.running && !ssn.connected);
  $("#chat-dot").className = "dot " + (any && !pending ? "live" : view.active ? "connecting" : "");
  const parts = [sourceLabel(view.channel ? `#${view.channel}` : "Twitch", tw), sourceLabel("Social Stream Ninja", ssn)].filter(Boolean);
  const idle = [tw.enabled ? (view.channel ? `#${view.channel}` : "Twitch") : null, ssn.enabled ? "Social Stream Ninja" : null].filter(Boolean).join(" + ") || "Aucune source";
  $("#chat-status").textContent = view.active ? parts.join("  |  ") : idle;
  $("#chat-status").title = [tw.detail, ssn.detail].filter(Boolean).join(" — ");
  const btn = $("#chat-connect");
  btn.textContent = view.active ? "Déconnecter" : "Connecter";
  btn.classList.toggle("accent", !view.active);
  const input = $("#chat-input");
  const can = (tw.connected && tw.writable) || ssn.connected;
  input.disabled = !can;
  $("#chat-send").disabled = !can;
  input.placeholder = can ? "Écrire dans le chat…" : any ? "Lecture seule : ajoute un compte et un jeton Twitch, ou Social Stream Ninja, dans Réglages → Chat" : "Chat déconnecté";
}

export function setState(v) {
  const wasActive = view.active;
  view = v;
  renderStatus();
  renderRequests();
  if (wasActive !== view.active && !items.length) renderLog();
}

export function setHistory(v) {
  items = (v.items || []).map((e) => e);
  renderLog();
}

export function onEvent(e) {
  if (e.kind === "message" || e.kind === "notice") {
    append(e);
  } else if (e.kind === "clear") {
    items = e.login ? items.filter((m) => m.kind !== "message" || m.login.toLowerCase() !== e.login.toLowerCase()) : [];
    renderLog();
  } else if (e.kind === "delete") {
    items = items.filter((m) => m.id !== e.id);
    renderLog();
  }
}

export function init() {
  document.addEventListener("blast:ptab", (e) => {
    visible = e.detail === "chat";
    if (visible) {
      unread = 0;
      renderUnread();
      const log = $("#chat-log");
      log.scrollTop = log.scrollHeight;
      if (!view.active && !view.twitch.enabled && !view.ssn.enabled) toast("info", "Choisis une source de chat dans Réglages → Chat, puis Connecter.");
    }
  });
  $("#chat-connect").addEventListener("click", () => send({ type: view.active ? "chat_disconnect" : "chat_connect" }));
  $("#chat-settings").addEventListener("click", () => openSettings("chat"));
  $("#chat-form").addEventListener("submit", (e) => {
    e.preventDefault();
    const input = $("#chat-input");
    const text = input.value.trim();
    if (!text) return;
    send({ type: "chat_say", text });
    input.value = "";
  });
  renderStatus();
  renderRequests();
  renderLog();
}
