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
