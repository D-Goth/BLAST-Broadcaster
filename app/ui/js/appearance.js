import { $, T, invoke, toast, setResult, formValues } from "./core.js";
import { themeColors } from "./theme.js";

let view = { settings: { glass: false, blur: 18, opacity: 0.55, dim: 0.25, background: null }, image: null };
let saveTimer = null;

// [UI-85]
function rgba(color, alpha) {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec((color || "").trim());
  if (!m) return color;
  let hex = m[1];
  if (hex.length === 3) hex = [...hex].map((c) => c + c).join("");
  const n = parseInt(hex, 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

function luminance(color) {
  const m = /^#?([0-9a-f]{6})$/i.exec((color || "").trim());
  if (!m) return 0;
  const n = parseInt(m[1], 16);
  return (0.299 * ((n >> 16) & 255) + 0.587 * ((n >> 8) & 255) + 0.114 * (n & 255)) / 255;
}

// [UI-86]
function backdrop(colors) {
  if (view.image) return `url("${T.core.convertFileSrc(view.image)}")`;
  const accent = colors.accent || "#FF1654";
  const second = colors.deck_b || "#888888";
  return `radial-gradient(circle at 18% 22%, ${rgba(accent, 0.55)}, transparent 45%), radial-gradient(circle at 82% 78%, ${rgba(second, 0.35)}, transparent 50%), linear-gradient(135deg, ${colors.bg || "#101010"}, ${colors.panel_alt || "#202020"})`;
}

// [UI-87]
export function applyAppearance() {
  const s = view.settings;
  const colors = themeColors();
  const root = document.documentElement;
  root.classList.toggle("glass", s.glass);
  root.style.setProperty("--glass-backdrop", backdrop(colors));
  root.style.setProperty("--glass-blur", `${s.blur}px`);
  root.style.setProperty("--glass-dim", String(s.dim));
  const light = luminance(colors.bg) > 0.5;
  root.style.setProperty("--glass-edge", light ? "rgba(255, 255, 255, 0.6)" : "rgba(255, 255, 255, 0.12)");
  root.style.setProperty("--glass-shadow", light ? "rgba(255, 255, 255, 0.7)" : "rgba(0, 0, 0, 0.55)");
  if (colors.text_muted && colors.text) {
    root.style.setProperty("--text-muted", s.glass ? `color-mix(in srgb, ${colors.text_muted} 60%, ${colors.text})` : colors.text_muted);
  }
  const panels = { bg: [colors.bg, s.opacity * 0.6], panel: [colors.panel, s.opacity], "panel-alt": [colors.panel_alt, Math.min(0.95, s.opacity + 0.1)] };
  for (const [name, [color, alpha]] of Object.entries(panels)) {
    if (color) root.style.setProperty(`--${name}`, s.glass ? rgba(color, alpha) : color);
  }
}

function renderForm() {
  const s = view.settings;
  const f = $("#appearance-form").elements;
  f.glass.checked = s.glass;
  f.blur.value = s.blur;
  f.opacity.value = s.opacity;
  f.dim.value = s.dim;
  $("#ap-blur-v").textContent = `${Math.round(s.blur)} px`;
  $("#ap-opacity-v").textContent = `${Math.round(s.opacity * 100)} %`;
  $("#ap-dim-v").textContent = `${Math.round(s.dim * 100)} %`;
  $("#bg-name").textContent = view.image ? "Image personnalisée" : "Sans image : dégradé aux couleurs du thème";
  $("#bg-clear").disabled = !view.image;
}

function setView(v) {
  view = v;
  applyAppearance();
  renderForm();
}

// [UI-88]
function editLive() {
  const v = formValues($("#appearance-form"));
  view.settings = { ...view.settings, glass: v.glass, blur: Number(v.blur), opacity: Number(v.opacity), dim: Number(v.dim) };
  applyAppearance();
  renderForm();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try {
      view = await invoke("save_appearance", { appearance: view.settings });
      setResult("#appearance-result", "ok", "Enregistré.");
    } catch (e) {
      setResult("#appearance-result", "err", String(e));
    }
  }, 300);
}

// [UI-89]
async function chooseImage() {
  const picked = await T.dialog.open({ multiple: false, directory: false, title: "Image de fond", filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "avif"] }] });
  if (!picked) return;
  setResult("#appearance-result", "busy", "Copie de l'image…");
  try {
    setView(await invoke("set_background", { path: picked }));
    setResult("#appearance-result", "ok", "Image appliquée.");
  } catch (e) {
    setResult("#appearance-result", "err", String(e));
  }
}

export async function loadAppearance() {
  try {
    setView(await invoke("get_appearance"));
  } catch (e) {
    toast("error", String(e));
  }
}

export function init() {
  const form = $("#appearance-form");
  form.addEventListener("input", editLive);
  form.addEventListener("submit", (e) => e.preventDefault());
  $("#bg-choose").addEventListener("click", chooseImage);
  $("#bg-clear").addEventListener("click", async () => {
    try {
      setView(await invoke("clear_background"));
    } catch (e) {
      toast("error", String(e));
    }
  });
}
