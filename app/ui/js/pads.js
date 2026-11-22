import { $, $$, S, send, invoke, toast, pickAudio } from "./core.js";

// [UI-60]
export const PADS_PER_PAGE = 12;
let pads = { pads: [], count: 36 };
let page = 0;
let menuPad = null;

const padAt = (index) => pads.pads.find((x) => x.index === index);
export const currentPage = () => page;

function padColor(i) {
  const palette = ["var(--accent)", "var(--deck-b)", "var(--meter-mid)", "var(--meter-low)"];
  return palette[Math.floor(i / 4) % palette.length];
}

// [UI-61]
function padButton(index, k) {
  const p = padAt(index);
  const el = document.createElement("button");
  el.className = "pad" + (p ? "" : " vacant") + (p && !p.ready && !p.error ? " loading" : "") + (p && p.error ? " error" : "");
  el.dataset.index = index;
  el.style.setProperty("--pad-color", (p && p.color) || padColor(k));
  el.title = p ? `${p.name}\n${p.path}${p.error ? "\n⚠ " + p.error : ""}\nClic droit : modifier` : "Pad vide : clic pour choisir un son, ou glisse un fichier ici";
  const label = document.createElement("span");
  label.textContent = p ? p.name : "+";
  const key = document.createElement("span");
  key.className = "key";
  key.textContent = `F${k + 1}`;
  el.append(label, key);
  return el;
}

// [UI-62]
function renderPads() {
  const pages = Math.max(1, Math.ceil(pads.count / PADS_PER_PAGE));
  page = Math.min(page, pages - 1);
  $("#pad-page").textContent = `${page + 1}/${pages}`;
  const box = $("#pads");
  box.innerHTML = "";
  for (let k = 0; k < PADS_PER_PAGE; k++) {
    const index = page * PADS_PER_PAGE + k;
    if (index >= pads.count) break;
    box.appendChild(padButton(index, k));
  }
  renderPadStates();
}

export function setPads(p) {
  pads = p;
  renderPads();
}

// [UI-63]
export function renderPadStates() {
  const mask = S.state ? BigInt(S.state.sfx || 0) : 0n;
  $$("#pads .pad").forEach((el) => {
    el.classList.toggle("playing", ((mask >> BigInt(el.dataset.index)) & 1n) === 1n);
  });
}

const pickSound = async () => (await pickAudio(false, "Choisir un son"))[0] || null;

// [UI-64]
export async function assignPad(index, path, name) {
  try {
    await invoke("save_pad", { pad: { index, name: name || "", path, color: "" } });
  } catch (e) {
    toast("error", String(e));
  }
}

// [UI-65]
export function playPad(index) {
  const known = !!padAt(index);
  if (known) send({ type: "pad_play", index });
  return known;
}

export function closePadMenu() {
  $("#pad-menu").classList.add("hidden");
  menuPad = null;
}

// [UI-66]
function openPadMenu(e, index) {
  menuPad = index;
  const p = padAt(index);
  $("#pad-name").value = p ? p.name : "";
  $("#pad-rename").disabled = !p;
  $("#pad-clear").disabled = !p;
  const menu = $("#pad-menu");
  menu.classList.remove("hidden");
  const r = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, window.innerWidth - r.width - 8)}px`;
  menu.style.top = `${Math.min(e.clientY, window.innerHeight - r.height - 8)}px`;
  $("#pad-name").focus();
}

function renameFromMenu() {
  const p = padAt(menuPad);
  const name = $("#pad-name").value.trim();
  closePadMenu();
  if (p && name) assignPad(p.index, p.path, name);
}

function turnPage(delta) {
  const pages = Math.ceil(pads.count / PADS_PER_PAGE);
  page = Math.max(0, Math.min(pages - 1, page + delta));
  renderPads();
}

export function init() {
  $("#pads").addEventListener("click", async (e) => {
    const el = e.target.closest(".pad");
    if (!el) return;
    const index = Number(el.dataset.index);
    if (!playPad(index)) {
      const path = await pickSound();
      if (path) assignPad(index, path, "");
    }
  });
  $("#pads").addEventListener("contextmenu", (e) => {
    const el = e.target.closest(".pad");
    if (!el) return;
    e.preventDefault();
    e.stopPropagation();
    openPadMenu(e, Number(el.dataset.index));
  });
  document.addEventListener("pointerdown", (e) => {
    if (menuPad !== null && !e.target.closest("#pad-menu")) closePadMenu();
  });
  $("#pad-pick").addEventListener("click", async () => {
    const index = menuPad;
    closePadMenu();
    const path = await pickSound();
    if (path) assignPad(index, path, "");
  });
  $("#pad-rename").addEventListener("click", renameFromMenu);
  $("#pad-name").addEventListener("keydown", (e) => {
    if (e.key === "Enter") renameFromMenu();
  });
  $("#pad-clear").addEventListener("click", () => {
    const index = menuPad;
    closePadMenu();
    invoke("clear_pad", { index }).catch((err) => toast("error", String(err)));
  });
  $("#pad-prev").addEventListener("click", () => turnPage(-1));
  $("#pad-next").addEventListener("click", () => turnPage(1));
  $("#sfx-stop").addEventListener("click", () => send({ type: "pad_stop_all" }));
  $("#sfx-vol").addEventListener("change", (e) => {
    invoke("save_sfx_volume", { volumeDb: Number(e.target.value) }).catch((err) => toast("error", String(err)));
  });
  renderPads();
}
