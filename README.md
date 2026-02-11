# Wuddle
Wuddle is a WoW DLL Updater (vibe-coded, to be clear) for client mods like Interact, nampower, SuperWoW, UnitXP_SP3, and more. The app's main use it to simplify keeping your mods up-to-date when playing on private servers without needing a special launcher.


## I've noticed that my app is causing a false positive in Windows Defender. I do not know how to fix this for the time being... 
I made this app in hopes of being able to make DLL management easier for more people than just myself, but this app is after all made for my own personal use, so when or if I'll be able to fix this, is uncertain.

<img width="524" height="383" alt="image" src="https://github.com/user-attachments/assets/8b13b7ef-ae24-4fcf-812a-27da9699c1a5" />

Reading up on this `Bearfoos.A!ml` online seems to indicate that his is a common issue with unknown Rust-based apps and is being caught by a machine-learning based pattern recognition, and considering that Wuddle's sole purpose is DLL mods, I'm not surprised it that may look sketchy from an AV's point of view.

## Update: I've removed some unused bundled in packages and so on, the app still false positive flags, but less so now. Here you can see the results:
VirusTotal and Hybrid-Analysis scans:
- [VirusTotal .zip scan](https://www.virustotal.com/gui/file/d60dd04c9dec3d1a042258e66082b7b339c36f1cc7ff63327404b10a03a253a0?nocache=1) 0/66 "Clean"
- [VirusTotal .exe scan](https://www.virustotal.com/gui/file/671fe8d50ef0b5618618a871bef90bb2ed2179ca02ba16408be8351077960e86) 1/65 AV softwares claim it's malware
- [Hybrid-Analysis Wuddle.exe scan](https://hybrid-analysis.com/sample/671fe8d50ef0b5618618a871bef90bb2ed2179ca02ba16408be8351077960e86) 85/100 threat score, AV scan says clean, but it ends up saying Malicious :/

##

Anyway, here's what the app looks like:
<img width="1104" height="727" alt="image" src="https://github.com/user-attachments/assets/085f1a95-f696-43be-86a6-d1e34a0df1f2" />
