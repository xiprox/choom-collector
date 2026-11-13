# 005: The app is named Choom Collector

The app, its repository, its program, and its Velopack pack ID are named Choom Collector, `choom-collector`, after Cyberpunk Co-op Tools until 0.3.0.
Decided by the user on 2026-11-13, at the end of the polish pass: "let's rename the app to 'Choom Collector'", then "Full rename" over renaming only what the window shows.

## Context

The app shipped 0.2.0 and 0.3.0 as Cyberpunk Co-op Tools, and only the user had installed it.
A name in the pack ID and the data folder costs every installed copy a reinstall when it changes, so it's cheapest before friends install the app.
A web search found nothing else named Choom Collector, for Cyberpunk 2077 or otherwise (2026-11-13); an earlier name, ChoomLink, was taken by a multiplayer mod.

## Options

- **Rename everything.** The window, the installer, the repository, the program, the pack ID, and the data folder take the new name; an installed copy can't update across the pack ID's change, so it's installed again once.
- **Rename only what players see.** Nothing breaks, but the program, the folders, and the repository keep a name the app no longer has.

## Decision

Everything is renamed: the repository is `xiprox/choom-collector`, the program `choom-collector`, the pack ID `choom-collector`, and the data folder `%APPDATA%\ChoomCollector\` on Windows and `~/.config/choom-collector/` on Linux.

- On its first start, the app moves the old data folder, `%APPDATA%\CyberpunkCoopTools\` or `~/.config/cyberpunk-coop-tools/`, to the new name, so settings, the code, and what a PC shares and follows carry over.
- The default logs folder becomes `Choom Collector Logs`; a logs folder setup saved keeps its path.
- The relay's Worker keeps its name, `cyberpunk-coop-tools-relay`: a new name would deploy a second Worker beside the one holding the user's domain, and only Cloudflare's dashboard shows it.
- The record's logs and the changelog keep the old name, as history.
