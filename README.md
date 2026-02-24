# Wuddle - WoW DLL Mods Manager

Wuddle is a desktop app that simplifies WoW DLL mods management, primarily for the Vanilla 1.12.1 client. 

A user-friendly app for installing DLL mods and keeping them up-to-date (checks for new Releases), Wuddle will also install companion addons if they're bundled with the DLL mod's download asset.
> [!WARNING]
> Please scroll down and read the Important Note before downloading.

<img width="1120" height="805" alt="image" src="https://github.com/user-attachments/assets/698f2f7b-0c4b-49be-8aa3-177431fad1de" />


## Features

- **Multi-forge support:** GitHub, Codeberg, Gitea, GitLab
- **Quick Add** Provides easy installation of commonly used Vanilla 1.12 mods
- **Custom Git URL support:** Add any DLL mods from the aforementioned git forges not included in the Quick Add section
- **Companion Addons:** Installs companion addons when bundled in mod zip (e.g. Interact, UnitXP_SP3)
- **Multi-instance/profile:** Add multiple WoW installs, each with its own tracked mod list
- **Manage `dlls.txt`:** Wuddle will update `dlls.txt` when adding, removing, or enabling/disabling mods
- **GitHub Token:** Optional GitHub auth token support to reduce anonymous API rate-limit issues

# Important Note (Anti-virus + SuperWoW)  
SuperWoW is known to cause false-positives in most anti-virus software.

Wuddle will show a warning before adding SuperWoW from Quick Add so users know to expect a false-positive appearing in their anti-virus software.  
This is not tied to the Quick Add or Wuddle itself, but rather an unavoidable issue for the time being, until SuperWoW stops triggering false-positives.

While Wuddle itself is not a malicious app of any kind, the actions it performs (downloading/removing/moving .dll files) appears suspicious from a malware scanner's perspective. 
I don't think I can ever make it not get at least a few false-positives or appearing suspicious to malware scanners.
Wuddle itself shouldn't trigger any warnings in Windows Defender unless SuperWoW is installed through it.  
This will make Defender or any other anti-virus software pinpoint Wuddle.exe as the culprit, since Wuddle is downloading and installing the client mod to your WoW directory.

For transparency, here are VirusTotal + Hybrid-Analysis scan results for the latest version (v1.0.6):
- Windows: Wuddle.exe
  - [VirusTotal Scan Results](https://www.virustotal.com/gui/file/b80eea0d8b1d10025cdfa5ceb718ab42da5b60682c5e6208618faf10cf2c320a/detection)
  
- Linux: wuddle-gui_1.0.6_amd64.appimage
  - [VirusTotal Scan Results](https://www.virustotal.com/gui/file/5fb985a1b954509f498e84f784d569f5ed2f8a06fa49a31049885b996ee825bb/detection)

- Hybrid-Analysis (Windows + Linux): [Scan Results](https://hybrid-analysis.com/file-collection/698f55aacef48b40b400f75b#)

## Supported Builds

- Linux: AppImage
- Windows: portable ZIP (`Wuddle.exe`, no installer)

## Credits / Inspiration

Wuddle is its own implementation, but several workflows and UX ideas were inspired by existing community projects:

- **GitAddonsManager** (WobLight)  - Big inspiration for solving multi-toc addons scenarios
  Git-based addon update workflow, `.toc`-driven addon folder handling, and branch-focused addon management ideas.  
  https://gitlab.com/woblight/GitAddonsManager

- **WoWRetroLauncher** (Parquelle)  - Vaguely inspired the UI redesign idea seen in Wuddle's v2.0 release.
  Reference for retro launcher visual direction and layout experimentation.  
  https://github.com/Parquelle/WoWRetroLauncher
