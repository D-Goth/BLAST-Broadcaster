import { $, $$, S, T, send } from "./core.js";
import { assignPad } from "./pads.js";

let drag = null;

function targetAt(x, y) {
  const el = document.elementFromPoint(x, y);
  if (!el) return null;
  const pad = el.closest(".pad");
  if (pad) return { kind: "pad", index: Number(pad.dataset.index), el: pad };
  const deck = el.closest(".deck");
  if (deck) return { kind: "deck", deck: deck.dataset.deck, el: deck };
  const row = el.closest(".row");
  if (row) {
    const r = row.getBoundingClientRect();
    const after = y > r.top + r.height / 2;
    return { kind: "row", index: Number(row.dataset.index) + (after ? 1 : 0), el: row, after };
  }
  if (el.closest(".playlist")) return { kind: "playlist", index: S.entries.length, el: $("#playlist") };
  return null;
}

function clearTargets() {
  $$(".drop-target").forEach((e) => e.classList.remove("drop-target"));
  $$(".drop-before, .drop-after").forEach((e) => e.classList.remove("drop-before", "drop-after"));
}

function showTarget(t) {
  clearTargets();
  if (!t) return;
  if (t.kind === "deck" || t.kind === "playlist" || t.kind === "pad") t.el.classList.add("drop-target");
  if (t.kind === "row") t.el.classList.add(t.after ? "drop-after" : "drop-before");
}

function dropEntry(t, id) {
  if (!t) return;
  if (t.kind === "deck") send({ type: "load_entry", id, deck: t.deck });
  else if (t.kind === "pad") {
    const e = S.entries.find((x) => x.id === id);
    if (e) assignPad(t.index, e.path, e.title);
  } else if (t.kind === "row" || t.kind === "playlist") send({ type: "move", id, to: t.index });
}

function dropFiles(t, paths) {
  if (!paths.length) return;
  if (t && t.kind === "pad") assignPad(t.index, paths[0], "");
  else if (t && t.kind === "deck") send({ type: "load_paths", deck: t.deck, paths });
  else send({ type: "add_paths", paths, index: t && t.kind === "row" ? t.index : null });
}

function initInternal() {
  const ghost = $("#ghost");
  $("#rows").addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || e.target.closest("button")) return;
    const row = e.target.closest(".row");
    if (row) drag = { id: Number(row.dataset.id), row, x: e.clientX, y: e.clientY, active: false };
  });
  document.addEventListener("pointermove", (e) => {
    if (!drag) return;
    if (!drag.active) {
      if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 6) return;
      drag.active = true;
      drag.row.classList.add("dragging-src");
      document.body.classList.add("dragging");
      ghost.textContent = $(".title", drag.row).textContent;
      ghost.style.display = "block";
    }
    ghost.style.left = `${e.clientX + 14}px`;
    ghost.style.top = `${e.clientY + 10}px`;
    showTarget(targetAt(e.clientX, e.clientY));
  });
  document.addEventListener("pointerup", (e) => {
    if (!drag) return;
    if (drag.active) dropEntry(targetAt(e.clientX, e.clientY), drag.id);
    drag.row.classList.remove("dragging-src");
    document.body.classList.remove("dragging");
    ghost.style.display = "none";
    clearTargets();
    drag = null;
  });
}

async function initOs() {
  const wv = T.webview && T.webview.getCurrentWebview ? T.webview.getCurrentWebview() : null;
  if (!wv || !wv.onDragDropEvent) return;
  await wv.onDragDropEvent((event) => {
    const p = event.payload;
    const dpr = window.devicePixelRatio || 1;
    const at = p.position ? targetAt(p.position.x / dpr, p.position.y / dpr) : null;
    if (p.type === "enter" || p.type === "over") {
      showTarget(at || { kind: "playlist", index: S.entries.length, el: $("#playlist") });
    } else {
      clearTargets();
      if (p.type === "drop") dropFiles(at, p.paths || []);
    }
  });
}

export async function init() {
  initInternal();
  await initOs();
}
