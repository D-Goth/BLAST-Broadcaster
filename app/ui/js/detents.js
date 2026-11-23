// [UI-150]
const SNAP_FIRST = 0.035;
const SNAP_OTHERS = 0.02;
let lists = 0;

// [UI-151]
function detentsOf(el) {
  return (el.dataset.detents || "")
    .split(",")
    .filter((s) => s.trim() !== "")
    .map(Number)
    .filter(Number.isFinite);
}

// [UI-152]
function snap(el) {
  const marks = detentsOf(el);
  if (!marks.length) return;
  const v = Number(el.value);
  const span = Number(el.max) - Number(el.min) || 1;
  marks.forEach((d, i) => {
    const zone = span * (i === 0 ? SNAP_FIRST : SNAP_OTHERS);
    if (v !== d && Math.abs(v - d) <= zone) el.value = d;
  });
}

// [UI-153]
function addTicks(el) {
  const marks = detentsOf(el);
  if (!marks.length || el.getAttribute("list")) return;
  const dl = document.createElement("datalist");
  dl.id = `detents-${++lists}`;
  for (const d of marks) {
    const o = document.createElement("option");
    o.value = d;
    dl.append(o);
  }
  document.body.append(dl);
  el.setAttribute("list", dl.id);
}

// [UI-154]
function reset(el) {
  const [first] = detentsOf(el);
  if (first === undefined || el.dataset.reset === "off") return;
  el.value = first;
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

export function init() {
  for (const el of document.querySelectorAll("input[type=range][data-detents]")) addTicks(el);
  const isDetent = (t) => t instanceof HTMLInputElement && t.type === "range" && t.dataset.detents;
  document.addEventListener("input", (e) => isDetent(e.target) && snap(e.target), true);
  document.addEventListener("dblclick", (e) => isDetent(e.target) && reset(e.target), true);
}
