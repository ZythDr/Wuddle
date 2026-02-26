# Wuddle

Wuddle is a desktop WoW launcher/manager focused on Vanilla-era clients (1.12.x), with support for:

- DLL mod management
- Git-based addon management (clone/pull workflow)
- Multi-instance profiles
- One-click game launch per instance

Current release line: **v2.x**.

## Screenshot

<!-- Replace this with your latest screenshot file/path -->
![Wuddle v2 screenshot](docs/wuddle-v2-screenshot.png)

## What’s New In v2

- Wuddle evolved from a DLL updater into a **WIP launcher + manager**
- Added **Addon management** with Git clone/pull updates and branch selection
- Added **Home tab** with update overview and launcher actions
- Added **PLAY flow** with per-instance launch methods (Auto/Lutris/Wine/Custom)
- Added **VanillaFixes-aware launch/install workflow**
- Added **multi-instance profile switching** and profile settings UI
- Added **theme system** (including retro/WoW-inspired themes)
- Added improved **search/filtering** for tracked mods/addons
- Added more robust **conflict handling** on addon install/update

## Core Features

- **Multi-forge support:** GitHub, Codeberg, Gitea, GitLab
- **DLL mod management:** install, update, reinstall/repair, remove
- **Addon git-sync mode:** track addon repos with clone/pull and branch selection
- **Quick Add catalog:** common Vanilla client mods with curated metadata
- **Companion addon links/info:** surfaced directly in quick-add entries
- **`dlls.txt` management:** enable/disable + sync behavior for DLL mods
- **Multi-instance profiles:** each profile has its own tracked mods/addons + launch config
- **GitHub auth token (optional):** helps avoid anonymous API limits
- **Logs panel:** operational visibility and copyable logs

## Important Note (Anti-virus + SuperWoW)

SuperWoW is known to trigger false-positives in many antivirus products.

Wuddle shows a warning before adding SuperWoW from Quick Add. If SuperWoW is installed through Wuddle, antivirus tools may attribute the detection to `Wuddle.exe` because Wuddle performs the download/install action.

## Supported Build Outputs

- Linux: AppImage, portable `.tar.gz`, `.deb`, `.rpm`
- Windows: portable `.zip` (`Wuddle.exe`, no installer)

## Credits / Inspiration

Wuddle is its own implementation, but parts of the workflow and UX were inspired by:

- **GitAddonsManager** (WobLight)  
  Git addon update workflows, `.toc`-driven addon deployment ideas, and branch-oriented addon management.  
  https://gitlab.com/woblight/GitAddonsManager

- **WoWRetroLauncher** (Parquelle)  
  Visual inspiration for retro launcher styling and layout exploration.  
  https://github.com/Parquelle/WoWRetroLauncher
