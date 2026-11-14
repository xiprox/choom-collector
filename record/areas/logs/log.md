# Logs log

## 2026-11-14: A tester on Linux sent the game folder's logs and the app's log, but none of the game's own logs their PC found

We asked how the app did on Linux in a real session, since every Proton path in [collecting logs](01-collecting.md) was unverified.
We expected the game folder's logs to arrive, and didn't know about the game's own logs and crash reports, which live inside the Proton prefix.

Bleack, a tester in the user's group, played the session of 2026-11-13 on CachyOS with Cyberpunk 2077 2.5 under Proton, running 0.3.0 and the updates after it (user, 2026-11-14).
We read the host's session folder on 2026-11-14 (measured on the host's Windows 11 PC):

- Bleack's `_peer.txt` says the PC picks up 142 files in the game folder, 0 crash reports, 3 of the game's own logs, and the app's log.
- The host holds 246 files under Bleack's `game/` and the app's log under `app/`, and no `localappdata/` or `crash-reports/` folder.
- Every other player's folder from the same session, the host's and two Windows testers', holds `localappdata/CrashInfo.json`, `localappdata/UserSettings.json`, and a `crash-reports/` folder.

So the 3 files the PC found never arrived.
TBD: whether they're older than 24 hours, which the first send leaves out, or the app read a Proton prefix other than the one the game runs in; their paths and dates on Bleack's PC settle it.

This led to the app's log naming each source's folder when a session starts, with its files and how many changed in the last 24 hours, as [sources and progress](01-collecting.md#sources-and-progress) describes, so Bleack's next session settles the TBD.
It affects [the v1 scope](../product/02-v1-scope.md#updates-and-platforms).

## 2026-11-13: In a real session, a tester's new line reached the host in 0.96s, a rotation was a rename, and a restart sent nothing twice

We asked whether stage 5's exit holds with the core carrying `coop-logs` over the relay: a line within 2s, a rotation renamed without sending it again, and nothing sent twice after a tester restarts.
We expected the line to take about a second, since a `Sender` looks at its files once a second.

A host and a tester, mira, ran from one copy of the debug build against `wrangler dev` on this PC, each with its own data folder, a made-up game folder with an empty `Cyberpunk2077.exe`, and `--local-app-data` pointing at an empty folder, so neither read the real game's files (measured on Windows 11, 2026-11-13):

- Within 6s of starting, the host's session folder had both players' redscript and red4ext logs, both app logs, and mira's `prices.csv` from the host's extra path `bin/x64/StreetVendors/*.csv`.
- A line appended to mira's redscript log was in the host's copy 0.96s later, timed by polling the copy every 50ms.
- Renaming mira's `red4ext.log` to `red4ext.log.1` and writing a new `red4ext.log` showed in mira's `_events.log` as one rotation and one new file, and the host's `red4ext.log.1` held the old content.
- After the tester's process was stopped and started again, `_events.log` said "Connected; 5 of their files are here already", and the only bytes the host gained were the one line written while the tester was down.
- The windows showed the counts per source, mira's "Up to date" in the status bar, and 0 files for the extra path on the host's own PC, in warning's color.

The host's own logs go through the relay's loop, so while the relay can't be reached they wait, and catch up once it's back.

## 2026-11-13: coop-logs ports CP77 Log Collector's collecting, and needs a generation per Welcome to resume exactly over the relay

We asked whether CP77 Log Collector's log collecting, ported to a `Sender` and a `Collector` that never touch the network, still follows files by content and resumes exactly, as [collecting logs](01-collecting.md) describes.
We expected the port to hold, and one gap the C# app never had: a tester keeps its socket while the host's drops, so a batch made before the host's new `Welcome` can reach the host after it.

`cargo test -p coop-logs` ran 11 unit tests and 14 tests in `src/logs/tests/collecting.rs`, and all 25 passed, the 14 in 0.84s (measured on Windows 11, 2026-11-13).
Each test in `collecting.rs` runs a `Sender` against a `Collector` with temporary folders, and moves the clock on by hand, so none waits for a scan:

- A growing log reached the host's copy byte for byte, and the host counted the same bytes the player sent.
- A rotation, `mod_settings.log` to `mod_settings.1.log` and a new `mod_settings.log`, renamed the host's copy and sent only the new file; a second rotation, through `.2`, did the same, and nothing went to `_history`.
- A copy under a new name was copied on the host with 0 bytes sent, and only what the copy gained afterwards was sent.
- A shrink, and a rewrite with the same first 600 bytes that only the fingerprint tells apart, each moved the old copy into `_history` and sent the new content.
- At the first scan, a log 2 days old wasn't sent until it grew, and its copy then started at its old end; a 35 MB log sent its last 32 MB, starting at a line start.
- After both sides restarted on the same folders, with one log grown and another rotated meanwhile, the player sent only the growth and the new file, and the host renamed the rotated copy.
- With a batch lost and a later one arriving after the host's new `Welcome`, the host's copies matched the player's files, with nothing in `_history`.
- Extra paths sent a `.csv` 3 days old and a folder's `.txt`, sent nothing for `../outside/*.txt` and reported 0 files for it, took a path the host added mid-session, and the counts reached the host.
- A crash report's `.txt` and `.json` were sent, and its `.dmp` and `.png` weren't.
- A host whose session folder sat inside the game folder didn't receive its own copies back.
- Two players named mira got `mira` and `mira-2`, and the second kept `mira-2` under a new name after the host restarted.
- A copy whose path passed 240 characters went to `_long/`, and `_manifest.tsv` named it.
- Progress went from 0 of 2 files to 1 of 2 to up to date.

Taking out the generation check made the lost-batch test fail: the host's copy of `x.log` no longer matched the player's file.
Taking out the session folder rule made the host receive its own copies, under `game/Co-op Logs/`.

What we changed from CP77 Log Collector, and why, as [collecting logs](01-collecting.md) marks each one:

- **Added:** every batch carries the generation of the latest `Welcome`, and the host drops any other, for the gap above.
- **Changed:** a folder holding `_collector.log` is skipped, where the C# app left out the host's output folder by path, since the core doesn't pass that path to the `Sender`.
- **Changed:** extra paths are the host's, relative to the game folder, refused when they'd leave it, and exempt from the 24-hour rule, since the host names them for every player.
- **Changed:** `.dmp` files are never sent, and the app's log has a source of its own, as [the v1 scope](../product/02-v1-scope.md#whats-picked-up) asks.
- **Changed:** a second player with a taken name gets a folder with `-2`, since the core lets both join, where the C# app refused one or mixed both into one folder.
- **Changed:** a tick sends at most 1 MB of each file and 2 MB in all, where the C# app sent 4 MB of each file per round and let TCP hold it back, since the core calls every 250ms with nothing to wait on.
- **Changed:** messages are postcard, a tick's go in batches under 300 KB, and `Welcome.logs` is compressed.
- **Left out:** the `Info` and `Ping` messages, the address, tool version, and machine name in `_peer.txt`, and the `Collector`'s copy of its events in the app's log, for the reasons the design's [Left out](01-collecting.md#left-out) gives.

This affects documents outside this area, which this change didn't edit: [the v1 scope](../product/02-v1-scope.md#what-the-host-gets) lists no `app/` folder and doesn't say extra paths skip the 24-hour rule, and [the record's index](../../index.md) lists neither [collecting logs](01-collecting.md) nor this log.
