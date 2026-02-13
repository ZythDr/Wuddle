# Wuddle - WoW DLL Mods Manager

Wuddle is a desktop app that simplifies WoW DLL mods management, primarily for the Vanilla 1.12.1 client. 

It gives a user-friendly GUI for adding mod repos, checking updates, installing/updating files, and it supports adding multiple instances of WoW.

<img width="1120" height="805" alt="image" src="https://github.com/user-attachments/assets/698f2f7b-0c4b-49be-8aa3-177431fad1de" />


## Features

- Multi-forge support:
  - GitHub
  - Codeberg
  - Gitea
  - GitLab
- Quick Add list for commonly used Vanilla 1.12 mods
- Custom Git URL support for any DLL mods not included in the Quick Add section
- Multiple WoW instances/profiles, each with its own tracked mod list
- Allows Enabling/disabling mods via `dlls.txt` entry toggling
- Optional GitHub auth token support to reduce anonymous API rate-limit issues

## Important Note (SuperWoW)

SuperWoW is known to cause antivirus false-positives on some systems.

Wuddle shows an in-app warning before adding SuperWoW from Quick Add so users know what to expect.

## Supported Builds

- Linux: AppImage
- Windows: portable ZIP (`Wuddle.exe`, no installer)
