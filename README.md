# Wuddle

Wuddle is a desktop WoW launcher/manager primarily focusing Vanilla clients, with support for:

- DLL mod management (install/update)
- Git-based addon management (inspired by [GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager))
- Multi-instance profiles
- One-click game launch per instance

> [!WARNING]
> Please scroll down and read the Important Note before downloading.

[Screencast_20260414_150926.webm](https://github.com/user-attachments/assets/231d99f9-1809-49e8-b6ba-6117876c08bc)


# Important Note (Anti-virus + SuperWoW)
SuperWoW is known to trigger false-positives in many antivirus products.
Wuddle shows a warning before adding SuperWoW from Quick Add. If SuperWoW is installed through Wuddle, antivirus tools may attribute the detection to `Wuddle.exe` because Wuddle performs the download/install action.

### Core Features

- **Addon management:** install, update, reinstall/repair, remove
- **DLL mod management:** install, update, reinstall/repair, remove
- **Multi-forge support:** GitHub, Codeberg, Gitea, GitLab
- **Quick Add catalog:** common Vanilla client mods with descriptions
- **Companion addon links/info:** shown directly in quick-add entries
- **`dlls.txt` management:** enable/disable mods in Wuddle without having to uninstall them
- **Multi-instance profiles:** each profile has its own tracked mods/addons + launch config
- **GitHub auth token (optional):** helps avoid anonymous API limits (60 per hour)
- **Logs panel:** operational visibility and copyable logs

### What's New in v3.2.0

#### New Features
- **Collection Addon Management** — Treat addon-git repositories as real collections, choose which addon folders to keep directly in the Add Repo preview, and manage installed collections later without re-adding the repo.
- **Nested Addon Discovery** — Wuddle now detects addon folders with `.toc` files up to 5 levels deep in addon-git repositories.

#### Improvements
- **Custom Executable Targeting** — Profiles can now target renamed or irregularly named game executables for Auto launch and Tweaks instead of only relying on `Wow.exe` or `VanillaFixes.exe`.
- **Targeted Tweaks Feedback** — Tweaks now reports which executable is being inspected and clearly explains when the selected client is not compatible with legacy 1.12.1 patching.

#### Bug Fixes
- **Collection Matching Fixes** — Fixed collection management for repositories whose folder names differ from the installed addon name, including common GitHub suffixes like `-master` and `-main`.
- **Nested Install Linking** — Fixed nested addon installs and repair flows so the correct repo-relative folder is linked or moved.

#### Removed
- **Legacy Radio UI** — Removed the in-app radio player and its related settings UI.
- **Turtle-Specific Home Links** — Removed the Turtle-only links section from the Home tab.
- **`I like turtles` Profile Flag** — Removed the old profile toggle that controlled Turtle-themed home content.

<details>
<summary><strong>v3.x Changelog</strong></summary>

### v3.1.0
- **Browse to Folder** — Quickly open the local folder for any tracked addon or mod directly from the UI.
- **Linux Stabilization** — Addon path tracking is now case-insensitive, preventing re-import issues and "ghost" entries on Linux filesystems.
- **Non-Blocking Rescan** — Broken path repair runs asynchronously during Rescan, preventing UI freezes during intensive repair operations.
- **Targeted Link Repair** — Normal refresh and launch only verify tracked addon links and repair broken entries on demand instead of scanning broadly.
- **Focused Startup Checks** — Automatic update checks now stay on the network/version path instead of running addon maintenance work first.
- **Rescan Phase Visibility** — Rescan now logs repair, cleanup, prune, import, and dedup phases with timing details in the Logs tab.
- **Cleaner Scans** — Improved manual scanning logic now ignores metadata and non-addon folders by strictly validating for `.toc` files.
- **Case-Insensitive Database** — Implemented `COLLATE NOCASE` in SQLite for repository lookups to prevent duplicate entries from varying URL casings.

### v3.0.7
- **API Transparency & Log Filtering** — Introduced a new `[API]` log category with a dedicated filter button and Cyan highlighting for technical budget tracking.
- **Immediate UI Refresh** — Successfully updated repositories are now instantly cleared from the Home tab's update list.
- **Restored Detailed Logging** — Verbose per-repository update reporting has been re-implemented for both single and bulk updates.

### v3.0.6
- **Token-Aware Update Checking** — Authenticated users now always perform full repository checks, while anonymous users benefit from optimized selective checks to stay within API limits.
- **Visual De-cluttering** — Removed "Infrequent Mod" indicators for authenticated users.
- **Reliability Fixes** — Corrected check timestamp logic to ensure fresh update results across both manual and auto-check modes.

### v3.0.5
- **Anti-Virus safety warnings** — Restored and generalized the warning dialog for mods known to trigger security heuristics (SuperWoW, VanillaFixes, UnitXP_SP3). 
- **Optimized Dialog Layout** — Increased warning dialog width to 650px for better readability and refined installation logic to prevent uninformed mod additions.

### v3.0.4

- **Turtle WoW background artwork** — Restored the Turtle WoW artwork background on the Home tab for when the "I like turtles" preset is enabled.
- **API Usage background sync** — Integrated a 60-second background subscription to monitor GitHub API rate limits, keeping rate-limit tooltips accurate without consuming user quota.


### v3.0.2

- **Live DLL Documentation** — Wuddle now pulls "live" documentation for individual WeirdUtils modules directly from the project's README on Codeberg. Usage instructions and commands will now always be up-to-date.
- **Improved Codebase Architecture** — Complete refactor into a professional, modular structure for better maintenance and transparency.
- **Fixed Scroll Stability** — Resolved a persistent issue where clicking inline code blocks in descriptions would trigger unwanted list scrolling.
- **Decoupled System Feedback** — Radio connection states and other background tasks are now decoupled from the main UI thread for a smoother experience.

### v3.0

Wuddle v3 is a complete frontend rewrite from Tauri/WebView to [Iced 0.14](https://iced.rs), rendering natively via wgpu (Vulkan/Metal/DX12). No WebView, no browser engine overhead.

- **In-game radio player** — stream the Everlook Broadcasting Co. radio with play/stop, volume controls (click-to-mute, scroll-to-adjust), auto-connect, auto-play, and configurable buffer via Radio Settings.
- **DXVK Configurator** — interactive `dxvk.conf` editor with per-setting tooltips, syntax-highlighted preview, and Turtle WoW-specific presets.
- **Version pinning** — per-mod dropdown to lock to a specific release tag while still tracking the latest version.
- **Merge updates mode** — per-repo toggle to keep existing files and only overwrite matching ones during updates.
- **DLL count mismatch warning** — prompts for Merge vs Clean update when the number of DLLs changes between releases.
- **Multi-DLL expand/collapse** — mods installing multiple DLLs appear as expandable parent rows with per-DLL enable/disable toggles.
- **Remove dialog with file preview** — scrollable file tree of every installed file before confirming removal.
- **GitHub-flavored admonitions** — README previews render `[!NOTE]`, `[!TIP]`, `[!WARNING]`, etc. with colored accents and icons.
- **Auto-scaling for smaller monitors** — detects monitor resolution and scales the UI automatically, with manual scale buttons (75%–120%) in Options.
- **GAM-compatible addon deployment** — addons are now cross-compatible with GitAddonsManager and the TurtleWoW launcher out of the box.
- **Mod cache in WoW directory** — simplifies antivirus whitelisting on Windows.

</details>

<details>
<summary><strong>v2.x Changelog</strong></summary>

### v2.5

- Bidirectional settings sync between Tauri and Iced frontends
- Release channel selector (Stable/Beta) with seamless v3 upgrade path
- Add dialog repo preview with README, file tree, and About panel
- Clickable file preview with syntax highlighting
- Addon deduplication with case-insensitive matching
- Fixed desktop notifications on Linux (D-Bus via notify-rust)
- Scroll-aware edge fading, sticky dialog footers
- Performance improvements (shared HTTP client, targeted updates, LRU caches)

### v2.4

- **Tweaks tab** — patch WoW.exe with quality-of-life improvements (FoV, Farclip, Quickloot, Camera fixes, etc.)
- Desktop notifications for mod/addon updates
- In-app changelog viewer
- Linux AppImage self-update
- Adaptive update frequency
- Auto-clear WDB cache, ignore updates per-repo
- Assets-pending detection for self-update

### v2.3

- Mod file integrity checking via SHA-256
- Automatic cache cleanup
- Addon conflict detection dialog
- Auto-check for updates with configurable interval
- Turtle WoW home section with community links
- Visual theme picker

### v2.0

- Evolved from DLL updater into launcher + manager
- Addon management with Git clone/pull and branch selection
- Home tab with PLAY button and per-instance launch methods
- Multi-instance profile switching
- Visual themes

</details>

## Credits / Inspiration

Wuddle is its own implementation, but parts of the functionality and UX were inspired by:

- **[GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager)** by WobLight  
  Git addon update workflows, `.toc`-driven addon deployment ideas, and branch-oriented addon management.  

  
- **[WoWRetroLauncher](https://github.com/Parquelle/WoWRetroLauncher)** by Parquelle  
  Sparked the idea for Wuddle's themes.  

  
- **[vanilla-tweaks](https://github.com/brndd/vanilla-tweaks)** by brndd  
  WoW.exe binary patching logic for the Tweaks tab (FoV, farclip, quickloot, camera fixes, etc.).  

  
