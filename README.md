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

### What’s New In v2

- Wuddle evolved from a DLL updater into a **WIP launcher + manager**
- Added **Addon management** with Git clone/pull updates and branch selection
- Added **Home tab** with update overview and launcher actions
- Added **PLAY button** with per-instance launch methods (Auto/Lutris/Wine/Custom)
- Added the ability to easily install **VanillaFixes** through the mods tab's quick-add section.
- Added **multi-instance profile switching** and profile settings UI
- Added **themes** (including a WoW UI inspired theme which is horrible, i wouldn't recommend it)
- Added improved **search/filtering** for tracked mods/addons
- Added more robust **conflict handling** on addon install/update ([GitAddonsManager](https://gitlab.com/woblight/GitAddonsManager) inspired)


## Credits / Inspiration

Wuddle is its own implementation, but parts of the functionality and UX were inspired by:

- **GitAddonsManager** (WobLight)  
  Git addon update workflows, `.toc`-driven addon deployment ideas, and branch-oriented addon management.  
  https://gitlab.com/woblight/GitAddonsManager

- **WoWRetroLauncher** (Parquelle)  
  Sparked the idea for Wuddle's themes.  
  https://github.com/Parquelle/WoWRetroLauncher
