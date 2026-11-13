# v1 scope

```text
Purpose:     What each feature of v1 does, the rules for its behavior, and where v1 stops.
Depends on:  01-aims.md
Decides:     Which of CP77 Log Collector's behaviors v1 keeps, changes, or drops, and what it adds.
```

v1 does what CP77 Log Collector 1.3.1 does for sessions, logs, and mods, minus the connection options the relay makes unnecessary, plus the delivery state a sharer needs and a setup done once.
Each behavior below that comes from the C# app is read in its README or source, cited once per section, and each change is marked as one.

## Sessions

Read in CP77 Log Collector, README.md, "Use" and "Getting the others connected", and src/Relay.cs, src/Sender.cs, and src/Settings.cs.
The relay's limits, codes, and timings are in [relay and sessions](../network/01-relay-and-sessions.md).

- A session is a host and the testers who joined through the host's relay with the host's code.
- The host's code is generated once and kept in the settings, so testers type it once.
- **Changed:** setup happens once.
  The first run asks for Host or Join, the relay's address, and for a tester the code; a pasted `relay.example.net/K7R-M2Q` fills both.
  From then on the app hosts or joins by itself every time it starts, and a tester whose host isn't up yet waits and joins when the host appears.
- The player's name comes from Steam, the `PersonaName` of the most recent account in Steam's `config/loginusers.vdf`, or else the system's user name.
  **Changed:** it isn't editable.
- A tester whose link drops tries again every 5s, or after 1s when the host asks it to.
  **Changed:** testers retry quickly at first and then every 8s, as [relay and sessions](../network/01-relay-and-sessions.md#reconnecting) describes.
  After a reconnect, a tester resumes from what the host has, so nothing is sent twice and nothing is lost.
- **Dropped:** the direct TCP connection on port 47077, UPnP, and the per-session choice of relay ([decision 003](../../decisions/003-relay-forked-from-fsc-editor.md)).
- **Added:** every player reports their game version, and a tester whose version differs from the host's is told so.

## Logs

Read in CP77 Log Collector, README.md, "What it collects" and "What the host gets", src/Sender.cs, and src/Common.cs.

### What's picked up

| Where | What |
| --- | --- |
| The game folder | every `*.log` and `*.log.N` at any depth, and `*.txt` in folders named `logs` |
| `REDEngine/ReportQueue` in the game's `%LOCALAPPDATA%` | crash reports' text files |
| `CD Projekt Red/Cyberpunk 2077` in the game's `%LOCALAPPDATA%` | `*.log` and `*.json`, such as CrashInfo and UserSettings |
| The app's data folder | the app's own log; **changed:** CP77 Log Collector keeps it next to its program |
| Extra paths | what the host lists, relative to the game folder |

On Linux the game runs in Proton, and its `%LOCALAPPDATA%` is inside the prefix Proton keeps for the game, `steamapps/compatdata/1091500/pfx/`.

- **Changed:** extra paths are one setting of the host's, sent to every tester, and written relative to the game folder, such as `bin/x64/StreetVendors/*.csv`, so they hold on every PC, Linux included.
- **Dropped:** each player's own options for history, crash dumps, and extra paths.
  Crash dumps (`.dmp`) are never sent, since Claude can't read a minidump without tools, and they're large (unverified: we haven't measured one of the game's).

### How they're sent

- When a tester joins, they send the last 32 MB of every log that changed in the last 24 hours, and after that everything new as it's written.
  Files under the host's extra paths go whatever their age, since the host named them.
- A file is followed by its content, not its name: its first 512 bytes, and a fingerprint of what was sent, FNV-1a over the bytes when there are 64 KB or less, or over 32 samples of 64 bytes spread across them.
  So when a mod rotates its logs, `x.log` to `x.1.log` and a new `x.log`, the host renames its copies instead of receiving them again, and a file copied under a new name is copied on the host.
- A full scan runs every 10s, and a rename, shrink, or delete triggers one at once.
- Data travels in blocks of up to 256 KB, compressed, and logs shrink about 10 times.

### What the host gets

The host's folder holds one folder per session, named by date and code, with a folder per player inside it:

```text
<logs folder>/<YYYY-MM-DD> <code>/
  _status.txt          who is connected, files, data, last activity, every 5s
  _collector.log       everything that happened, every player
  <player>/
    game/...           the same layout as their game folder
    crash-reports/...
    localappdata/...
    extra/...
    app/app.log        this app's own log on their PC
    _peer.txt          their game version, system, and connection state
    _events.log        new files, rotations, reconnects
    _manifest.tsv      which part of which source file each copy holds, and where it is here
    _history/<time>/   older copies a rotation or a rewrite replaced
    _long/             copies whose full path would be too long for Windows
```

- Copies are plain files, readable while they're being written.
- **Changed:** the folder is named by session, so one folder holds one test run (CP77 Log Collector writes into one folder the host picks).
- **Dropped:** the player's game folder in `_peer.txt`, since its path can hold their user name, and the host's analysis hasn't needed it.
- TBD: whether the session folder gets an index written for Claude, naming who played, on which game version and build, and when anyone crashed; stage 5 left it open, for the first session with friends to show whether Claude needs one.

## Mods

Read in CP77 Log Collector, README.md, "Sharing mods", and src/Mods.cs.

### Finding mods

A mod is a top-level folder in one of the game's mod places, and folders with the same name, capitals aside, in different places are one mod.

| Place | Type |
| --- | --- |
| `bin/x64/plugins/cyber_engine_tweaks/mods` | CET mod |
| `red4ext/plugins` | RED4ext plugin |
| `r6/scripts` | redscript |
| `r6/tweaks` | TweakXL |
| `r6/input` | Input Loader |
| `archive/pc/mod` | archive |
| `mods` | REDmod |

- Only folders can be shared.
  A loose file named after a mod, such as `archive/pc/mod/EquipmentEx.archive` beside the `EquipmentEx` folders, is pointed out, since moving it into a folder of that name in the same place makes it part of the mod.

### Sharing and following

- Sharing is one click on a mod in the player's list.
  A folder has one sharer at a time: the first to share it keeps it until they stop or leave, and anyone else who offered theirs takes over then.
- Following is one click on a shared mod.
  The follower's folders become exact copies of the sharer's: a changed file arrives within a couple of seconds, a file the sharer no longer has is deleted, and a file that's already the same is never sent, everything compared by SHA-256.
- A new file is written beside its place and renamed over it, so the game never reads half a file.
  On Windows, a file the running game holds is replaced once the game lets go of it.
- When a sharer adds a folder to a mod a player follows, the follow goes half-ticked, and nothing new arrives until the player agrees.
- A follow belongs to the PC the player agreed to take the folder from, so when someone else shares it, nothing changes until the player ticks it again.
- An edit in a followed folder is undone within seconds.
  To take a folder over, the sharer stops sharing, the other player shares, and the first follows them; their copies already match, so nothing is sent.
- Following asks first, and asks more strongly when the mod has DLLs, since mods run code inside the game.
- Unfollowing asks whether to delete the copy too.
  Deleting removes all its folders, after copying them to the backups; a folder that's a link to one elsewhere loses only the link.
  On Windows the game may hold some files, so the app offers to try again once the game is closed.
- **Added:** each follower reports their state for each mod they follow, and the sharer sees it: up to date, fetching n of m files, needs a game restart, or paused while reconnecting.

### What stays out

- Files the game or a mod writes while it runs are never sent and never deleted from a follower's copy: `*.log *.log.* logs/ db.sqlite3 *.sqlite3-journal *.tmp *.bak .git/ __folder_managed_by_vortex *.vortex_backup install-id.txt name.txt`, and the app's own temporary files.
- Each sharer can add patterns for their own folders in Settings, and followers go by them.
- Of two names that differ only in capitals, only one is shared.

### Checks

- The host takes a folder only from a path inside one of the mod places, with no `..`, drive letters, or odd characters, and a file only from that folder's sharer, with the size and hash they listed.
- Followers check every path again before writing anything.
- On Linux, a folder that already exists with different capitals is used as it is.

### Through the host

- Players can't reach each other, so a file goes up from its sharer once, into the host's cache, and on from there to everyone who wants it, even while it's still arriving.
- The cache keeps a file for 10 minutes after its last use, and empties when the host stops.
- If a sharer or the host goes away, everyone's copies stay as they are, and a file on its way picks up where it stopped.

### Backups

What following replaces or deletes, or unfollowing deletes, goes to the backups in the app's data folder, by time: up to 2 GB in all, the oldest going first, and no single file over 512 MB.

## Updates and platforms

- The app updates itself from the project's GitHub releases with Velopack, delta packages included ([decision 004](../../decisions/004-releases-with-release-please-and-velopack.md)).
- Windows 10 and 11 on x64, and Linux on x64.
  CI builds Linux and runs every test on it, but no release has run on a real Linux desktop or a Steam Deck, since nobody in the user's group plays on Linux (user, 2026-11-13).
- On Linux the app finds the game in every Steam library, for native, Flatpak, and Snap Steam, and in Heroic and Lutris installs (read in CP77 Log Collector, README.md, "Linux").
- TBD: whether v1 keeps CP77 Log Collector's mode without a window (`--nogui`), which it offers for Linux PCs missing X11 libraries; the user hasn't said.
