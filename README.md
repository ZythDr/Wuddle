# Wuddle

Wuddle is a desktop WoW launcher/manager primarily focusing Vanilla clients, with support for:

- DLL mod management (install/update)
- Git-based addon management (inspired by [GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager))
- Multi-instance profiles
- One-click game launch per instance

> [!WARNING]
> Please scroll down and read the Important Note before downloading.

<img width="1099" height="904" alt="image" src="https://github.com/user-attachments/assets/b827cd42-7b6c-47b2-b85b-01b75f171665" />

# Important Note (Anti-virus + SuperWoW)
SuperWoW is known to trigger false-positives in many antivirus products.
Wuddle shows a warning before adding SuperWoW from Quick Add. If SuperWoW is installed through Wuddle, antivirus tools may attribute the detection to `Wuddle.exe` because Wuddle performs the download/install action.

### Core Features

- **Multi-forge support:** GitHub, Codeberg, Gitea, GitLab
- **DLL mod management:** install, update, reinstall/repair, remove
- **Addon git-sync mode:** track addon repos with clone/pull and branch selection
- **Quick Add catalog:** common Vanilla client mods with curated metadata
- **Companion addon links/info:** surfaced directly in quick-add entries
- **`dlls.txt` management:** enable/disable + sync behavior for DLL mods
- **Multi-instance profiles:** each profile has its own tracked mods/addons + launch config
- **GitHub auth token (optional):** helps avoid anonymous API limits
- **Logs panel:** operational visibility and copyable logs

### What's New In v2.5.6

- **Forge icon + Release Notes in Add dialog:** Previewing a repo now shows forge icon and Release Notes buttons, matching the detail dialog.
- **Markdown code blocks:** Fenced (`` ``` ``) and inline (`` ` ``) code blocks now render correctly in README previews from Gitea/GitLab repos.
- **Fixed README links:** Links in README previews now open in the system browser and show the correct URL on right-click.
- **Clearable inputs:** All text inputs now have a clear button (✕). Added DMA-BUF rendering toggle for Linux.

<details>
<summary><strong>Previous versions</strong></summary>

### What's New In v2.5.5

- **Fixed GIF playback speed:** Animated GIFs in README previews now play at their intended frame rate — fixes WebKitGTK playing some GIFs extremely fast.
- **Debounced search inputs:** Project and log search now wait 500ms after typing stops before updating, reducing unnecessary re-renders.

### What's New In v2.5.4

- **Fixed desktop notifications:** Notifications now work reliably on Linux using native D-Bus, with the Wuddle app icon and correct app name.
- **Notifications on manual check:** "Check for updates" now always sends a desktop notification with the result.
- **Fixed Lutris launch in AppImage:** All launch modes now clean inherited AppImage/Tauri environment variables and detach the game into its own process group — fixes Lutris failures in AppImage builds and prevents the game from showing Wuddle's taskbar icon.

### What's New In v2.5.3

- **Clickable file preview:** Click any file in the Installed Files or repo file tree to preview its contents directly in Wuddle — works for Lua, XML, TOC, Markdown, CSS, JS, and plain text files.
- **Syntax highlighting:** File previews include language-aware highlighting with a VS Code-inspired color theme.
- **"Changelog" → "Release Notes":** Renamed and simplified to show only forge release entries. Mods now default to the Release Notes view.
- **Repo name casing fix:** Display names now match the actual repository casing. Existing repos lowercased by the v2.5.2 migration are automatically corrected on first startup.
- **Symlink-safe file reading:** Local file previews no longer break when the WoW directory is a symlink.

### What's New In v2.5.2

- **Addon deduplication:** Wuddle now prevents duplicate addon entries when multiple instances (or forks) manage the same addon directory. Repos are matched case-insensitively, folder ownership is checked during import, and stale fork entries are automatically cleaned up on startup.
- **README media support:** Images and videos in repo README previews now display correctly in the Add dialog.
- **Responsive side panel:** The About/Files panel in the Add dialog shrinks gracefully on narrow windows before the main content area.
- **Addon-friendly Add dialog:** Clearer subtitle text and contextual placeholder URLs for the addon Add flow.
- **Changelog rendering fix:** `###` headers now render correctly in the in-app changelog viewer.

### What's New In v2.5

- **Add dialog repo preview:** Pasting a repo URL now shows the README, a file tree (expandable one level), and an About panel (description, stars, forks, language, license) — all fetched live from GitHub/GitLab/Gitea.
- **Quick Add + README shared frame:** Presets and README share a single content region with a swappable header, reducing clutter.
- **Advanced mode toggle:** A footer checkbox hides the install mode dropdown by default for a cleaner add flow.
- **Scroll-aware edge fading:** All scrollable frames now fade at the top/bottom edges to indicate overflow, with theme-aware colors that update on theme switch.
- **Sticky dialog footers:** Instance Settings, Changelog, Addon Conflict, and SuperWoW Warning dialogs use consistent head/body/foot layouts with non-scrolling footers.
- **Performance improvements:** Shared HTTP client, targeted branch-dropdown updates (no more UI freeze on 60+ addon repos), consolidated DOM observers, and LRU-capped caches.

### What's New In v2.4

- **Tweaks tab (vanilla-tweaks integration):** Patch WoW.exe directly from Wuddle with quality-of-life improvements based on [vanilla-tweaks by brndd](https://github.com/brndd/vanilla-tweaks). Includes:
  - **Widescreen FoV** — wider field of view for widescreen monitors (with degree display)
  - **Farclip / Frilldistance** — adjustable terrain and grass render distances
  - **Nameplate Distance** — extended nameplate visibility range
  - **Camera Skip Fix** — eliminates camera skip/jitter when rotating
  - **Max Camera Distance** — configurable zoom-out limit
  - **Sound in Background** — keep game audio when alt-tabbed
  - **Sound Channels** — increase simultaneous audio channels
  - **Quickloot (Reverse)** — auto-loot by default, hold Shift for manual
  - **Large Address Aware** — allow WoW.exe to use up to 4 GB of memory
- **Read Current:** extract and display actual tweak values from WoW.exe
- **Reset to Default:** one-click restore to recommended settings
- **Automatic backup:** WoW.exe.bak is created before the first patch; one-click restore available
- **Per-profile tweak settings:** each instance remembers its own tweak configuration

### What's New In v2.3

- **Mod file integrity checking:** Wuddle now detects when mod files have been modified outside the app. Modified mods show a warning badge and are skipped during bulk updates — click the download button to restore to the latest version.
- **Automatic cache cleanup:** old cached mod versions are pruned after each install. Configurable in Options (0–10 versions to keep, default 3). The launcher also cleans up old `Wuddle-bin.exe` versions on self-update.
- **Addon conflict detection:** adding an addon that shares folders with an already-tracked repo now shows a conflict dialog before proceeding.
- **Auto-check for updates:** optional background polling with configurable interval (enabled by default).
- **Turtle WoW home section:** curated official and community links, toggled per instance. Adaptive column layout.
- **Visual theme picker:** color swatches replace the old dropdown.
- **Ignore errored repos:** right-click menu item to dismiss errored mods/addons with an "Ignored" badge.
- **AV false-positive tags:** VanillaFixes and UnitXP_SP3 presets now display antivirus warnings.
- **UI polish:** relocated spinner, enlarged title, profile picker hidden when only one instance exists, search clears on tab switch.
- **Linux fixes:** links now open correctly in AppImage builds; spinner uses theme color.
- **Zip security fix:** path traversal vulnerability patched during extraction.
- **Product rename:** app now displays as "Wuddle" everywhere (About page, desktop entries, bundles).

### What's New In v2

- Wuddle evolved from a DLL updater into a **WIP launcher + manager**
- Added **Addon management** with Git clone/pull updates and branch selection
- Added **Home tab** with update overview and launcher actions
- Added **PLAY button** with per-instance launch methods (Auto/Lutris/Wine/Custom)
- Added the ability to easily install **VanillaFixes** through the mods tab's quick-add section.
- Added **multi-instance profile switching** and profile settings UI
- Added **themes** (including a WoW UI inspired theme which is horrible, i wouldn't recommend it)
- Added improved **search/filtering** for tracked mods/addons
- Added more robust **conflict handling** on addon install/update ([GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager) inspired)

</details>

## Credits / Inspiration

Wuddle is its own implementation, but parts of the functionality and UX were inspired by:

- **[GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager)** by WobLight  
  Git addon update workflows, `.toc`-driven addon deployment ideas, and branch-oriented addon management.  

  
- **[WoWRetroLauncher](https://github.com/Parquelle/WoWRetroLauncher)** by Parquelle  
  Sparked the idea for Wuddle's themes.  

  
- **[vanilla-tweaks](https://github.com/brndd/vanilla-tweaks)** by brndd  
  WoW.exe binary patching logic for the Tweaks tab (FoV, farclip, quickloot, camera fixes, etc.).  

  
