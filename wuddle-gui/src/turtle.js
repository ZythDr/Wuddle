// Turtle WoW section: event listener bindings for all Turtle-related home buttons.

import {
  TURTLE_ADDONS_URL,
  TURTLE_HOMEPAGE_URL,
  TURTLE_TIMERS_URL,
  TURTLE_DATABASE_URL,
  TURTLE_FORUM_URL,
  RETROCRO_MODS_URL,
  TURTLE_TALENT_CALC_URL,
  TURTLE_DISCORD_URL,
  WOWAUCTIONS_URL,
} from "./state.js";

import { $ } from "./utils.js";
import { openUrl } from "./repos.js";

function updateLinkRows() {
  const section = $("homeTurtleSection");
  if (!section || section.classList.contains("hidden")) return;
  // Measure the first column to figure out available space for link buttons.
  const col = section.querySelector(".home-turtle-col");
  if (!col) return;
  const subhead = col.querySelector(".home-turtle-subhead");
  const subheadH = subhead ? subhead.offsetHeight : 0;
  // Card padding (14px top + 14px bottom) + grid margin-top 2px + col gap 8px + grid gap
  const cardPad = 14 * 2;
  const gridMargin = 2;
  const colGap = 8; // gap between subhead and links
  const overhead = cardPad + gridMargin + subheadH + colGap;
  const available = section.clientHeight - overhead;
  if (available <= 0) return;
  for (const container of section.querySelectorAll(".home-turtle-links")) {
    const count = container.children.length;
    if (!count) continue;
    const btn = container.children[0];
    const rowH = btn.offsetHeight + 8; // 8px gap between buttons
    const rows = Math.max(1, Math.floor((available + 8) / rowH));
    container.style.setProperty("--link-rows", String(Math.min(rows, count)));
  }
}

let _turtleRo = null;
export function observeTurtleResize() {
  const section = $("homeTurtleSection");
  if (!section || _turtleRo) return;
  _turtleRo = new ResizeObserver(() => updateLinkRows());
  _turtleRo.observe(section);
  updateLinkRows();
}

export function bindTurtleListeners() {
  $("homeBtnTurtleHomepage").addEventListener("click", async () => {
    await openUrl(TURTLE_HOMEPAGE_URL);
  });
  $("homeBtnTurtleDatabase").addEventListener("click", async () => {
    await openUrl(TURTLE_DATABASE_URL);
  });
  $("homeBtnTurtleForum").addEventListener("click", async () => {
    await openUrl(TURTLE_FORUM_URL);
  });
  $("homeBtnTurtleTalentCalc").addEventListener("click", async () => {
    await openUrl(TURTLE_TALENT_CALC_URL);
  });
  $("homeBtnTurtleDiscord").addEventListener("click", async () => {
    await openUrl(TURTLE_DISCORD_URL);
  });
  $("homeBtnTurtleAddons").addEventListener("click", async () => {
    await openUrl(TURTLE_ADDONS_URL);
  });
  $("homeBtnTurtleTimers").addEventListener("click", async () => {
    await openUrl(TURTLE_TIMERS_URL);
  });
  $("homeBtnRetroCroMods").addEventListener("click", async () => {
    await openUrl(RETROCRO_MODS_URL);
  });
  $("homeBtnWowAuctions").addEventListener("click", async () => {
    await openUrl(WOWAUCTIONS_URL);
  });
}
