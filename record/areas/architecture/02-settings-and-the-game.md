# Settings and the game

```text
Purpose:     What the app keeps between runs, and how it finds the game, its version, the player's name, and the system.
Depends on:  01-spine.md, product/02-v1-scope.md
Decides:     The settings and their file, what the first run asks, and the order the app looks for the game in.
```

The app keeps one `settings.toml` in its data folder, written by the first run's setup and read on every start, so a player who set up once hosts or joins by themselves from then on.
Everything else it needs, the game, its version, the name, and the system, it finds on its own at each start.

## Settings

| Setting | What it holds |
| --- | --- |
| `role` | `host` or `tester`, or nothing until setup finishes |
| `relay` | The relay's address as the player entered it, such as `relay.example.net` |
| `code` | The code this PC hosts, or the one it joins, without the hyphen |
| `resume` | The relay's resume token for the code this PC hosts |
| `logs_folder` | Where the host's logs go; the default is `Cyberpunk Co-op Tools Logs` in the player's Documents folder |
| `game_folder` | The game folder setup confirmed or the player chose |
| `install_id` | 32 hex digits picked once per install, so the host knows a returning tester as the same player |
| `extra_paths` | The host's extra paths, relative to the game folder |

- The app writes the file beside a temporary copy and renames it over the old one, so a crash mid-write leaves the old settings.
- Changing the relay or the code clears the resume token, since it proves the host only for the code it came with.
- Stop and Leave show setup with the answers so far, and keep the settings, so the next start hosts or joins again.

## Finding the game

The app looks in this order, and takes the first folder with `bin/x64/Cyberpunk2077.exe` in it (read in CP77 Log Collector, src/Settings.cs, `GameFinder`):

1. The folder setup saved, if it still has the game.
2. The app's own folder and the 5 above it, for an app dropped into the game folder.
3. `steamapps/common/Cyberpunk 2077` in every Steam library: Steam's own folder, from the registry on Windows and from native, Flatpak, and Snap Steam on Linux, and every `path` in its `steamapps/libraryfolders.vdf`.
4. On Windows, GOG's install record in the registry, and the Epic launcher's manifests that mention the game.
5. On Linux, Heroic's and Lutris's usual folders.

- **The version** is the game program's `ProductVersion`, read from its version resource with `pelite`, which works on Linux too, since Proton runs the same Windows program; it read `2.5` on the development machine (measured on Windows 11, 2026-11-13).
- **The store** is told from the folder: Steam under `steamapps`, GOG under a `GOG` or `Heroic` folder, Epic under an `Epic` folder.
- **The name** is the `PersonaName` of the account Steam marks `MostRecent` in its `config/loginusers.vdf`, or else the system's user name.
- **The system** is Windows 11 when Windows' build number is 22000 or more, since Windows 11 still calls itself Windows 10 in `ProductName`, and on Linux the `PRETTY_NAME` in `/etc/os-release`.
- **The game's `%LOCALAPPDATA%`**, where crash reports and the game's own logs live, is the player's own on Windows, and on Linux the `AppData/Local` of `steamuser` or another user inside the game's Proton prefix, `steamapps/compatdata/1091500/pfx`, or a Heroic prefix.
