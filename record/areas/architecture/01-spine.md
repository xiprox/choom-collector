# The spine

```text
Purpose:     How the app's code splits into crates and threads, how the window learns what the rest is doing, and where its files live.
Depends on:  product/02-v1-scope.md
Decides:     The crates and what each holds, what runs on which thread, how state reaches the window and commands leave it, and the data folder.
```

The app is one program, `cyberpunk-coop-tools`, built from five crates in one Cargo workspace.
Only the window's crate compiles gpui, so the four that hold the logic build and test without it.
The logic runs on a tokio runtime of its own, and the window gets a fresh snapshot of the state whenever it changes, and sends back commands.

## Crates

| Crate | Folder | Holds | Uses |
| --- | --- | --- | --- |
| `cyberpunk-coop-tools` | `src/app` | The window, the theme, the command line, and the program's entry point | `coop-core` |
| `coop-core` | `src/core` | Settings, finding the game, the relay client, the session and its players, and passing messages to logs and mods | `coop-proto`, `coop-logs`, `coop-mods` |
| `coop-logs` | `src/logs` | Finding and following logs on a player's PC, and writing them into the host's folder | `coop-proto` |
| `coop-mods` | `src/mods` | Finding mods, their manifests, sharing and following, the host's cache, and applying a follow with backups | `coop-proto` |
| `coop-proto` | `src/proto` | The messages players exchange, and their encoding | nothing of ours |

`coop-logs` and `coop-mods` take and give messages, and never touch the network, so their tests run without a relay.
The program's manifest is the workspace's root `Cargo.toml`, since release-please versions the root package.

## Threads

- The window runs on gpui's main thread, and never waits on a file or the network.
- `coop-core` starts a tokio runtime on threads of its own, for the relay's WebSocket, timers, and scanning, with blocking file work on tokio's blocking pool.
- `coop-core` keeps one `State`: the session, its players, the mods, and what's being sent and received.
  When it changes, `coop-core` sends a snapshot of it to the window over an `async-channel`, at most every 100ms, and the window keeps the newest one and redraws.
  The spike showed a worker thread updating the window this way every second ([interface log](../interface/log.md#2026-11-13-gpui-kit-builds-every-control-the-window-needs-on-windows-and-the-exe-is-1665-mb)).
- The window sends commands back over a second channel: share, follow, unfollow, copy the invite, stop, leave, and save settings.

```text
 gpui main thread                       coop-core's tokio runtime
 ┌──────────────┐   Command   ───────▶  ┌─────────────────────────────────┐
 │   window     │                       │ session ── relay WebSocket       │
 │ (latest      │  ◀─────── Snapshot    │    ├── coop-logs (scan, follow)  │
 │  snapshot)   │    of State, ≤10/s    │    └── coop-mods (share, apply)  │
 └──────────────┘                       └─────────────────────────────────┘
```

## Files on disk

| What | Windows | Linux |
| --- | --- | --- |
| The data folder | `%APPDATA%\CyberpunkCoopTools\` | `~/.config/cyberpunk-coop-tools/` |
| Settings | `settings.toml` in the data folder | the same |
| The app's own log | `app.log` in the data folder, moved to `app.1.log` at 5 MB | the same |
| Backups of replaced and deleted mod files | `mod-backups/` in the data folder | the same |
| The host's cache of mod files on their way | `mod-cache/` in the data folder | the same |
| The host's logs | the folder chosen at setup | the same |

`--data-dir <folder>` puts the data folder elsewhere for one run, which every test run uses, so tests never touch the real settings.

## The command line

| Option | What it does |
| --- | --- |
| `--data-dir <folder>` | Uses this data folder instead of the usual one. |
| `--demo` | Runs a made-up session that shows the states the frames show, without a relay. |
| `--name <name>` | Shows up as this name instead of the Steam name, for test runs of several copies on one PC. |
| `--local-app-data <folder>` | Reads this folder as the game's `%LOCALAPPDATA%`, so a test run keeps off the real game's crash reports and logs. |

`tools/capture-window.ps1` starts the program with any of these, saves a picture of its window, and closes it, without taking the focus, so layout checks run on Windows while someone uses the PC.

Velopack's install, update, and uninstall hooks run before anything else in `main`, as [Velopack's Rust guide](https://docs.velopack.io/getting-started/rust) requires.
