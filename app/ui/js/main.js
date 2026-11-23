import { $, S, listen, toast, invoke } from "./core.js";
import * as theme from "./theme.js";
import * as decks from "./decks.js";
import * as playlist from "./playlist.js";
import * as dnd from "./dnd.js";
import * as cast from "./cast.js";
import * as settings from "./settings.js";
import * as voice from "./voice.js";
import * as detents from "./detents.js";
import * as pads from "./pads.js";
import * as obs from "./obs.js";
import * as shortcuts from "./shortcuts.js";
import * as update from "./update.js";
import * as jingles from "./jingles.js";
import * as appearance from "./appearance.js";
import * as rotation from "./rotation.js";
import * as icecast from "./icecast.js";
import * as chat from "./chat.js";

// [UI-15]
function tickClock() {
  $("#clock").textContent = new Date().toLocaleTimeString("fr-FR", { hour12: false });
}

// [UI-16]
async function subscribe() {
  await listen("blast://state", (e) => {
    S.state = e.payload;
    decks.renderState();
  });
  await listen("blast://playlist", (e) => {
    S.entries = e.payload.entries;
    playlist.renderPlaylist(e.payload.total);
  });
  await listen("blast://blocked", (e) => playlist.setBlocked(e.payload.blocked));
  await listen("blast://notice", (e) => toast(e.payload.level, e.payload.message));
  await listen("blast://obs", (e) => obs.setObs(e.payload));
  await listen("blast://history", (e) => playlist.setHistory(e.payload));
  await listen("blast://pads", (e) => pads.setPads(e.payload));
  await listen("blast://jingles", (e) => jingles.setJingles(e.payload));
  await listen("blast://library", (e) => rotation.setLibrary(e.payload));
  await listen("blast://cast", (e) => cast.setCast(e.payload));
  await listen("blast://chat-state", (e) => chat.setState(e.payload));
  await listen("blast://chat-event", (e) => chat.onEvent(e.payload));
  await listen("blast://chat-history", (e) => chat.setHistory(e.payload));
}

// [UI-17]
async function start() {
  for (const m of [detents, theme, decks, playlist, cast, settings, voice, pads, jingles, obs, shortcuts, update, appearance, rotation, icecast, chat]) m.init();
  $("#themes-folder").addEventListener("click", async () => {
    try {
      const dir = await invoke("themes_dir");
      toast("info", `Dépose tes thèmes .toml dans : ${dir} — puis relance BLAST.`);
    } catch (e) {
      toast("error", String(e));
    }
  });
  setInterval(tickClock, 500);
  tickClock();
  await subscribe();
  await appearance.loadAppearance();
  await theme.loadThemes();
  try {
    const view = await invoke("get_settings");
    voice.applySettings(view);
    jingles.applySettings(view);
  } catch (e) {
    toast("error", String(e));
  }
  await dnd.init();
  await invoke("blast", { cmd: { type: "refresh" } }).catch((e) => toast("error", String(e)));
  update.checkOnStart();
}

start();
