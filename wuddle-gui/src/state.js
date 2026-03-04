export const WOW_KEY = "wuddle.wow_dir";
export const PROFILES_KEY = "wuddle.profiles";
export const ACTIVE_PROFILE_KEY = "wuddle.profile.active";
export const TAB_KEY = "wuddle.tab";
export const PROJECT_VIEW_BY_PROFILE_KEY = "wuddle.project_view.by_profile";
export const OPT_SYMLINKS_KEY = "wuddle.opt.symlinks";
export const OPT_XATTR_KEY = "wuddle.opt.xattr";
export const OPT_CLOCK12_KEY = "wuddle.opt.clock12";
export const OPT_THEME_KEY = "wuddle.opt.theme";
export const OPT_FRIZ_FONT_KEY = "wuddle.opt.frizfont";
export const OPT_AUTOCHECK_KEY = "wuddle.opt.autocheck";
export const OPT_AUTOCHECK_MINUTES_KEY = "wuddle.opt.autocheck.minutes";
export const OPT_CACHE_KEEP_KEY = "wuddle.opt.cache.keep.versions";
export const IGNORED_ERRORS_KEY = "wuddle.ignored.errors";
export const LOG_WRAP_KEY = "wuddle.log.wrap";
export const LOG_AUTOSCROLL_KEY = "wuddle.log.autoscroll";
export const LOG_LEVEL_KEY = "wuddle.log.level";
export const WUDDLE_REPO_URL = "https://github.com/ZythDr/Wuddle";
export const WUDDLE_RELEASES_URL = "https://github.com/ZythDr/Wuddle/releases";
export const WUDDLE_RELEASES_API_URL = "https://api.github.com/repos/ZythDr/Wuddle/releases/latest";
export const TURTLE_ADDONS_URL = "https://turtle-wow.fandom.com/wiki/Addons#Full_Addons_List";
export const TURTLE_HOMEPAGE_URL = "https://turtlecraft.gg/";
export const TURTLE_DATABASE_URL = "https://database.turtlecraft.gg/";
export const TURTLE_FORUM_URL = "https://forum.turtlecraft.gg/";
export const TURTLE_TIMERS_URL = "https://turtletimers.com/";
export const RETROCRO_MODS_URL = "https://github.com/RetroCro/TurtleWoW-Mods";
export const TURTLE_TALENT_CALC_URL = "https://talents.turtlecraft.gg/";
export const TURTLE_DISCORD_URL = "https://discord.gg/turtlewow";
export const WOWAUCTIONS_URL = "https://www.wowauctions.net/";
export const MAX_PARALLEL_UPDATES = 5;
export const DEFAULT_THEME_ID = "cata";
export const DEFAULT_USE_FRIZ_FONT = true;
export const DEFAULT_AUTO_CHECK_ENABLED = true;
export const DEFAULT_AUTO_CHECK_MINUTES = 60;
export const DEFAULT_CACHE_KEEP_VERSIONS = 3;
export const MIN_AUTO_CHECK_MINUTES = 1;
export const MAX_AUTO_CHECK_MINUTES = 240;
export const SELF_UPDATE_POLL_MINUTES = 30;
export const SUPPORTED_THEMES = new Set(["cata", "obsidian", "emerald", "ashen", "wowui"]);

export const state = {
  repos: [],
  plans: [],
  planByRepoId: new Map(),
  branchOptionsByRepoId: new Map(),
  branchOptionsLoading: new Set(),
  openMenuRepoId: null,
  tab: "home",
  pending: 0,
  refreshInFlight: null,
  removeTargetRepo: null,
  removeTargetProfile: null,
  githubAuth: null,
  initialAutoCheckDone: false,
  loggedNoTokenAutoSkip: false,
  filter: "all",
  projectSearchQuery: "",
  sortKey: "name",
  sortDir: "asc",
  lastCheckedAt: null,
  clock12: false,
  theme: DEFAULT_THEME_ID,
  useFrizFont: DEFAULT_USE_FRIZ_FONT,
  autoCheckEnabled: DEFAULT_AUTO_CHECK_ENABLED,
  autoCheckMinutes: DEFAULT_AUTO_CHECK_MINUTES,
  cacheKeepVersions: DEFAULT_CACHE_KEEP_VERSIONS,
  autoCheckTimerId: null,
  lastUpdateNotifyKey: "",
  lastSelfUpdateNotifyVersion: "",
  nextSelfUpdatePollAt: 0,
  logLines: [],
  logLevel: "all",
  logQuery: "",
  logAutoScroll: true,
  logWrap: false,
  logRenderQueued: false,
  logDirty: false,
  profiles: [],
  activeProfileId: "default",
  projectViewByProfile: {},
  projectView: "mods",
  instanceSettingsDraft: null,
  authHealthSeenSession: false,
  authHealthActiveIssue: "",
  presetExpanded: new Set(),
  aboutInfo: null,
  aboutLoaded: false,
  aboutRefreshedAt: null,
  aboutLatestVersion: null,
  aboutSelfUpdate: null,
  aboutSelfUpdateBusy: false,
  launchDiagnostics: null,
  ignoredErrorRepoIds: new Set(),
};

