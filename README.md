# Wuddle - WoW DLL Mods Manager

Wuddle is a desktop app that simplifies DLL client-mod management for World of Warcraft 1.12.1.

It provides a user-friendly GUI for adding mod repos, checking updates, installing/updating files, and managing multiple WoW instances.

<img width="1120" height="805" alt="Wuddle UI" src="https://github.com/user-attachments/assets/698f2f7b-0c4b-49be-8aa3-177431fad1de" />

## Features

- Multi-forge support:
  - GitHub
  - Codeberg
  - Gitea
  - GitLab
- Quick Add list for commonly used Vanilla 1.12 mods
- Custom Git URL support for mods not included in Quick Add
- Multiple WoW instances/profiles, each with its own tracked mod list
- Per-mod status:
  - Up to date
  - Update available
  - Fetch error
  - Disabled
- Enable/disable mods by toggling entries in `dlls.txt`
- Remove from Wuddle only, or remove and delete local installed files
- Optional GitHub auth token support to reduce anonymous API rate limits
- Built-in logs panel with copy/clear/search/filter
- About tab with runtime/platform diagnostics (including backend detection)

## Important Note (SuperWoW)

SuperWoW is known to trigger antivirus false-positives on some systems.

Wuddle shows an in-app warning before adding SuperWoW from Quick Add so users know what to expect.

## Supported Builds

- Linux: AppImage
- Windows: portable ZIP (`Wuddle.exe`, no installer)

## Project Structure

- `wuddle-engine/`: Rust core engine
- `wuddle-gui/`: Tauri GUI (HTML/CSS/JS + Rust host)

## Local Development

```bash
cd wuddle-gui
npm ci
npm run tauri dev
```

## Build Locally

```bash
cd wuddle-gui
npx tauri build
```

## Release Workflow (GitHub Actions)

Release builds are handled by `.github/workflows/release-build.yml`.

When you push a tag like `v1.0.4`:

- Linux AppImage is built
- Windows portable ZIP is built
- A GitHub Release is published with both artifacts

Version syncing is automated in CI:

- Tag `vX.Y.Z` -> app version becomes `X.Y.Z` in build metadata/artifacts
