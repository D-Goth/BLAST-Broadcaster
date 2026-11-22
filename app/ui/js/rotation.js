import { $, $$, T, send, invoke, toast, setResult, formValues, fillForm } from "./core.js";

const DAYS = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];
const PALETTE = ["#FF1654", "#1F6FEB", "#D9730D", "#0E9F6E", "#7A5AF8", "#E5B800", "#00B3C7", "#B4436C"];

let rot = { categories: [], clocks: [], schedule: [], default_clock: null };
let library = { dirs: [], tracks: 0, scanning: false, scanned: 0 };
let selCat = null;
let selClock = null;
let paint = null;
let painting = false;
let countTimer = null;

const uid = (p) => `${p}-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
const words = (t) => t.split(",").map((w) => w.trim()).filter(Boolean);
const num = (v) => (v === "" || v == null || isNaN(Number(v)) ? null : Number(v));
const pickColor = (list) => PALETTE[list.length % PALETTE.length];
const errorOut = (e) => toast("error", String(e));

// [UI-90]
function showRotPage(page) {
  $$("[data-rot]").forEach((b) => b.classList.toggle("active", b.dataset.rot === page));
  $$("[data-rotpage]").forEach((p) => (p.hidden = p.dataset.rotpage !== page));
  if (page === "schedule") renderSchedule();
}

// [UI-91]
export function setLibrary(v) {
  library = v;
  const box = $("#lib-dirs");
  box.innerHTML = "";
  if (!library.dirs.length) {
    const e = document.createElement("div");
    e.className = "muted rot-empty";
    e.textContent = "Aucun dossier : ajoute le dossier de ta musique.";
    box.appendChild(e);
  }
  library.dirs.forEach((d, i) => {
    const row = document.createElement("div");
    row.className = "rot-item";
    row.innerHTML = `<span class="rot-name"></span><button type="button" class="btn ghost small">Retirer</button>`;
    $(".rot-name", row).textContent = d;
    $("button", row).addEventListener("click", () => saveDirs(library.dirs.filter((_, j) => j !== i)));
    box.appendChild(row);
  });
  $("#lib-status").textContent = library.scanning
    ? `Lecture en cours… ${library.scanned} titre(s) lus`
    : `${library.tracks} titre(s) en bibliothèque`;
  $("#lib-rescan").disabled = library.scanning || !library.dirs.length;
}

async function saveDirs(dirs) {
  try {
    await invoke("save_library_dirs", { dirs });
  } catch (e) {
    errorOut(e);
  }
}

function dot(color) {
  const d = document.createElement("span");
  d.className = "rot-dot";
  d.style.background = color || "var(--text-muted)";
  return d;
}

// [UI-92]
function renderCategories() {
  const box = $("#cat-list");
  box.innerHTML = "";
  for (const c of rot.categories) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "rot-item" + (c.id === selCat ? " active" : "");
    row.append(dot(c.color), Object.assign(document.createElement("span"), { className: "rot-name", textContent: c.name || "Sans nom" }));
    row.addEventListener("click", () => {
      selCat = c.id;
      renderCategories();
    });
    box.appendChild(row);
  }
  const cat = rot.categories.find((c) => c.id === selCat);
  $("#cat-form").hidden = !cat;
  if (!cat) return;
  const r = cat.rule;
  fillForm($("#cat-form"), {
    name: cat.name,
    color: cat.color || "#FF1654",
    k_morceau: r.kinds.includes("morceau"),
    k_jingle: r.kinds.includes("jingle"),
    k_pub: r.kinds.includes("pub"),
    genres: r.genres.join(", "),
    year_min: r.year_min ?? "",
    year_max: r.year_max ?? "",
    bpm_min: r.bpm_min ?? "",
    bpm_max: r.bpm_max ?? "",
    folders: r.folders.join("\n"),
  });
  refreshCount();
}

// [UI-93]
function readCategory() {
  const cat = rot.categories.find((c) => c.id === selCat);
  if (!cat) return;
  const v = formValues($("#cat-form"));
  cat.name = v.name.trim();
  cat.color = v.color;
  cat.rule = {
    kinds: ["morceau", "jingle", "pub"].filter((k) => v[`k_${k}`]),
    genres: words(v.genres),
    year_min: num(v.year_min),
    year_max: num(v.year_max),
    bpm_min: num(v.bpm_min),
    bpm_max: num(v.bpm_max),
    folders: v.folders.split("\n").map((f) => f.trim()).filter(Boolean),
  };
  const item = $("#cat-list .rot-item.active");
  if (item) {
    $(".rot-name", item).textContent = cat.name || "Sans nom";
    $(".rot-dot", item).style.background = cat.color;
  }
  refreshCount();
}

function refreshCount() {
  clearTimeout(countTimer);
  countTimer = setTimeout(async () => {
    const cat = rot.categories.find((c) => c.id === selCat);
    if (!cat) return;
    try {
      const [n] = await invoke("rotation_counts", { rules: [cat.rule] });
      $("#cat-count").textContent = `${n} titre${n > 1 ? "s" : ""} correspondent`;
    } catch (e) {
      $("#cat-count").textContent = "";
    }
  }, 250);
}

// [UI-94]
function renderClocks() {
  const box = $("#clock-list");
  box.innerHTML = "";
  for (const k of rot.clocks) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "rot-item" + (k.id === selClock ? " active" : "");
    row.append(dot(k.color), Object.assign(document.createElement("span"), { className: "rot-name", textContent: `${k.name || "Sans nom"} · ${k.slots.length} case(s)` }));
    row.addEventListener("click", () => {
      selClock = k.id;
      renderClocks();
    });
    box.appendChild(row);
  }
  const clock = rot.clocks.find((k) => k.id === selClock);
  $("#clock-form").hidden = !clock;
  if (!clock) return;
  fillForm($("#clock-form"), { name: clock.name, color: clock.color || "#1F6FEB" });
  renderSlots(clock);
}

// [UI-95]
function renderSlots(clock) {
  const box = $("#clock-slots");
  box.innerHTML = "";
  if (!rot.categories.length) {
    box.textContent = "Crée d'abord une catégorie.";
    return;
  }
  clock.slots.forEach((id, i) => {
    const row = document.createElement("div");
    row.className = "clock-slot";
    const n = Object.assign(document.createElement("span"), { className: "slot-num", textContent: String(i + 1) });
    const sel = document.createElement("select");
    for (const c of rot.categories) sel.appendChild(Object.assign(document.createElement("option"), { value: c.id, textContent: c.name || "Sans nom" }));
    sel.value = id;
    const cat = rot.categories.find((c) => c.id === id);
    row.style.setProperty("--slot-color", (cat && cat.color) || "var(--border)");
    sel.addEventListener("change", () => {
      clock.slots[i] = sel.value;
      renderSlots(clock);
      renderClocks();
    });
    const mk = (label, title, run) => {
      const b = Object.assign(document.createElement("button"), { type: "button", className: "btn ghost small", innerHTML: label, title });
      b.addEventListener("click", run);
      return b;
    };
    const move = (d) => () => {
      const j = i + d;
      if (j < 0 || j >= clock.slots.length) return;
      [clock.slots[i], clock.slots[j]] = [clock.slots[j], clock.slots[i]];
      renderSlots(clock);
    };
    row.append(n, sel, mk("&#8593;", "Monter", move(-1)), mk("&#8595;", "Descendre", move(1)), mk("&#10005;", "Retirer", () => {
      clock.slots.splice(i, 1);
      renderClocks();
    }));
    box.appendChild(row);
  });
}

// [UI-96]
function renderSchedule() {
  const pal = $("#sched-palette");
  pal.innerHTML = "";
  const choices = [{ id: null, name: "Effacer", color: "transparent" }, ...rot.clocks];
  for (const k of choices) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "chip-btn sched-chip" + (paint === k.id ? " active" : "");
    b.append(dot(k.id ? k.color : "transparent"), document.createTextNode(k.name || "Sans nom"));
    b.addEventListener("click", () => {
      paint = k.id;
      renderSchedule();
    });
    pal.appendChild(b);
  }
  const def = $("#sched-default");
  def.innerHTML = "";
  def.appendChild(Object.assign(document.createElement("option"), { value: "", textContent: "Aucune (auto-mix sur la playlist seule)" }));
  for (const k of rot.clocks) def.appendChild(Object.assign(document.createElement("option"), { value: k.id, textContent: k.name }));
  def.value = rot.default_clock || "";
  const grid = $("#sched-grid");
  grid.innerHTML = "";
  grid.appendChild(document.createElement("span"));
  for (let h = 0; h < 24; h++) grid.appendChild(Object.assign(document.createElement("span"), { className: "sched-hour", textContent: String(h).padStart(2, "0") }));
  DAYS.forEach((day, d) => {
    grid.appendChild(Object.assign(document.createElement("span"), { className: "sched-day", textContent: day }));
    for (let h = 0; h < 24; h++) {
      const i = d * 24 + h;
      const cell = document.createElement("span");
      cell.className = "sched-cell";
      cell.dataset.slot = i;
      paintCell(cell, rot.schedule[i]);
      grid.appendChild(cell);
    }
  });
}

function paintCell(cell, id) {
  const k = id && rot.clocks.find((c) => c.id === id);
  cell.style.background = k ? k.color : "";
  cell.title = k ? k.name : "Hors planning";
  cell.classList.toggle("set", !!k);
}

function applyPaint(cell) {
  const i = Number(cell.dataset.slot);
  while (rot.schedule.length < 168) rot.schedule.push("");
  rot.schedule[i] = paint || "";
  paintCell(cell, paint);
}

// [UI-97]
export function applySettingsView(view) {
  rot = JSON.parse(JSON.stringify(view.rotation));
  if (!rot.categories.some((c) => c.id === selCat)) selCat = rot.categories[0] ? rot.categories[0].id : null;
  if (!rot.clocks.some((k) => k.id === selClock)) selClock = rot.clocks[0] ? rot.clocks[0].id : null;
  if (paint && !rot.clocks.some((k) => k.id === paint)) paint = null;
  renderCategories();
  renderClocks();
  renderSchedule();
}

// [UI-98]
async function save() {
  try {
    const view = await invoke("save_rotation", { rotation: rot });
    applySettingsView(view);
    setResult("#rotation-result", "ok", "Grilles enregistrées.");
  } catch (e) {
    setResult("#rotation-result", "err", String(e));
  }
}

async function pickDir(title) {
  const picked = await T.dialog.open({ directory: true, multiple: false, title });
  return picked || null;
}

export function init() {
  $$("[data-rot]").forEach((b) => b.addEventListener("click", () => showRotPage(b.dataset.rot)));
  $("#rotation-save").addEventListener("click", save);
  $("#lib-add").addEventListener("click", async () => {
    const d = await pickDir("Dossier de musique");
    if (d && !library.dirs.includes(d)) saveDirs([...library.dirs, d]);
  });
  $("#lib-rescan").addEventListener("click", () => send({ type: "library_rescan" }));
  $("#cat-add").addEventListener("click", () => {
    const c = { id: uid("cat"), name: `Catégorie ${rot.categories.length + 1}`, color: pickColor(rot.categories), rule: { kinds: ["morceau"], genres: [], year_min: null, year_max: null, bpm_min: null, bpm_max: null, folders: [] } };
    rot.categories.push(c);
    selCat = c.id;
    renderCategories();
  });
  $("#cat-form").addEventListener("input", readCategory);
  $("#cat-form").addEventListener("submit", (e) => e.preventDefault());
  $("#cat-folder").addEventListener("click", async () => {
    const d = await pickDir("Dossier de la catégorie");
    if (!d) return;
    const f = $("#cat-form").elements.folders;
    f.value = [f.value.trim(), d].filter(Boolean).join("\n");
    readCategory();
  });
  $("#cat-delete").addEventListener("click", () => {
    const used = rot.clocks.filter((k) => k.slots.includes(selCat)).map((k) => k.name);
    if (used.length) {
      toast("warn", `Catégorie utilisée par : ${used.join(", ")}. Retire-la de ces grilles d'abord.`);
      return;
    }
    rot.categories = rot.categories.filter((c) => c.id !== selCat);
    selCat = rot.categories[0] ? rot.categories[0].id : null;
    renderCategories();
  });
  $("#clock-add").addEventListener("click", () => {
    const k = { id: uid("grille"), name: `Grille ${rot.clocks.length + 1}`, color: pickColor(rot.clocks), slots: rot.categories.length ? [rot.categories[0].id] : [] };
    rot.clocks.push(k);
    selClock = k.id;
    renderClocks();
  });
  $("#clock-form").addEventListener("input", (e) => {
    if (e.target.tagName === "SELECT") return;
    const clock = rot.clocks.find((k) => k.id === selClock);
    if (!clock) return;
    const v = formValues($("#clock-form"));
    clock.name = v.name.trim();
    clock.color = v.color;
    const item = $("#clock-list .rot-item.active");
    if (item) {
      $(".rot-name", item).textContent = `${clock.name || "Sans nom"} · ${clock.slots.length} case(s)`;
      $(".rot-dot", item).style.background = clock.color;
    }
  });
  $("#clock-form").addEventListener("submit", (e) => e.preventDefault());
  $("#slot-add").addEventListener("click", () => {
    const clock = rot.clocks.find((k) => k.id === selClock);
    if (!clock || !rot.categories.length) return;
    clock.slots.push(clock.slots[clock.slots.length - 1] || rot.categories[0].id);
    renderClocks();
  });
  $("#clock-delete").addEventListener("click", () => {
    rot.clocks = rot.clocks.filter((k) => k.id !== selClock);
    rot.schedule = rot.schedule.map((id) => (id === selClock ? "" : id));
    if (rot.default_clock === selClock) rot.default_clock = null;
    selClock = rot.clocks[0] ? rot.clocks[0].id : null;
    renderClocks();
  });
  $("#sched-default").addEventListener("change", (e) => (rot.default_clock = e.target.value || null));
  const grid = $("#sched-grid");
  grid.addEventListener("pointerdown", (e) => {
    const cell = e.target.closest(".sched-cell");
    if (!cell) return;
    e.preventDefault();
    painting = true;
    applyPaint(cell);
  });
  grid.addEventListener("pointerover", (e) => {
    const cell = e.target.closest(".sched-cell");
    if (painting && cell) applyPaint(cell);
  });
  document.addEventListener("pointerup", () => (painting = false));
  showRotPage("library");
  setLibrary(library);
}

