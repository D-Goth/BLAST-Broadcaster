import { settingsOpen } from "./settings.js";
import { closePadMenu, playPad, currentPage, PADS_PER_PAGE } from "./pads.js";
import { pttDown, pttUp, toggleLock } from "./voice.js";

const typing = (e) => {
  const t = e.target;
  return t && (t.tagName === "INPUT" || t.tagName === "SELECT" || t.tagName === "TEXTAREA");
};

export function init() {
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closePadMenu();
    if (settingsOpen() || typing(e)) return;
    if (e.code === "Space") {
      e.preventDefault();
      if (!e.repeat) pttDown();
      return;
    }
    if (e.key === "m" || e.key === "M") {
      if (!e.repeat) toggleLock();
      return;
    }
    const f = /^F([1-9]|1[0-2])$/.exec(e.key);
    if (f) {
      e.preventDefault();
      if (!e.repeat) playPad(currentPage() * PADS_PER_PAGE + Number(f[1]) - 1);
    }
  });
  document.addEventListener("keyup", (e) => {
    if (e.code === "Space") {
      e.preventDefault();
      pttUp();
    }
  });
  window.addEventListener("contextmenu", (e) => e.preventDefault());
}
