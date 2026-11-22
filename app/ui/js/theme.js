import { $, invoke, store } from "./core.js";
import { renderState } from "./decks.js";
import { applyAppearance } from "./appearance.js";

let themes = [];
let current = null;

export const themeColors = () => (current && current.colors) || {};
export const view = { rainbow: false };

// [UI-18]
function applyTheme(id) {
  const theme = themes.find((t) => t.id === id) || themes[0];
  if (!theme) return;
  current = theme;
  const root = document.documentElement;
  for (const section of ["colors", "meters"]) {
    for (const [k, v] of Object.entries(theme[section] || {})) {
      root.style.setProperty(`--${k.replaceAll("_", "-")}`, v);
    }
  }
  const fx = theme.effects || {};
  root.style.setProperty("--rainbow-speed", `${3 / Math.max(0.1, fx.rainbow_speed || 1)}s`);
  $("#theme-select").value = theme.id;
  store.set("blast.theme", theme.id);
  if (store.get("blast.rainbow") === null) setRainbow(!!fx.deck_rainbow, false);
  applyAppearance();
}

// [UI-19]
function setRainbow(on, persist = true) {
  view.rainbow = on;
  $("#rainbow-toggle").classList.toggle("on", on);
  if (persist) store.set("blast.rainbow", on ? "1" : "0");
  renderState();
}

// [UI-20]
export async function loadThemes() {
  try {
    themes = await invoke("themes");
  } catch (_) {
    themes = [];
  }
  const sel = $("#theme-select");
  sel.innerHTML = "";
  for (const t of themes) {
    const o = document.createElement("option");
    o.value = t.id;
    o.textContent = (t.meta && t.meta.name) || t.id;
    sel.appendChild(o);
  }
  applyTheme(store.get("blast.theme") || "black-lab");
  const r = store.get("blast.rainbow");
  if (r !== null) setRainbow(r === "1", false);
}

export function init() {
  $("#theme-select").addEventListener("change", (e) => applyTheme(e.target.value));
  $("#rainbow-toggle").addEventListener("click", () => setRainbow(!view.rainbow));
}
