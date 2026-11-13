# Plan

The plan builds v1, as [the v1 scope](02-v1-scope.md) describes it, in stages that each end in something we can watch work.
Each stage writes the design docs it needs before its code, and the polish pass comes last, at the user's request.
A stage is done when a commit meets its exit criterion and adds a "Done on" line under it.

## 1. Record and workspace

The record, the Cargo workspace with the crates [the spine](../architecture/01-spine.md#crates) lists, and CI building the window on both platforms.

Exit: CI builds Windows and Linux from `main`, and the program opens an empty window on Windows.

## 2. The window, on made-up data

The theme and every screen of [the interface decisions](../interface/decisions.md), fed by `--demo`, a made-up session that shows the states the frames show, and `--data-dir <folder>`.

Exit: pictures from `tools/capture-window.ps1` of setup for joining and for hosting, and of the window as the host and as a tester, show every element the frames in `experiments/areas/interface/frames/` show.

Done on 2026-11-13: the pictures are in `experiments/areas/interface/stage-2/` ([interface log](../interface/log.md#2026-11-13-the-window-draws-every-frame-in-gpui-from-made-up-state)).

## 3. The relay

`relay/`, forked from fsc-editor's, carrying binary messages, with a deploy guide, and the relay client in `coop-core`.

Exit: under `wrangler dev`, a test's host and two testers exchange messages, the host hears a tester leave, and a host that drops and returns within 30s keeps its testers.

## 4. Sessions

Settings, the first run, hosting and joining on start, players and their state, and reconnecting, with the window on real state.

Exit: on one PC, a host and two testers, each with their own data folder, see each other in the window, and a tester that restarts rejoins with no action.

## 5. Logs

Collection as the scope describes it: sources, history, following by content, rotations, resuming, the host's folder, which logs each player's PC picks up, and extra paths from the host.

Exit: in a test session, a line a tester's log gains is in the host's copy within 2s, a rotation renames the host's copies without sending them again, and a tester that restarts sends nothing twice.

## 6. Mods

Sharing as the scope describes it: finding and merging, sharing and following, the host's cache, applying with backups, held files on Windows, the DLL warning, loose files, unfollowing, and each follower's state for the sharer.

Exit: in a test session, a file the sharer saves is in a follower's copy within 5s, a file a running program holds on Windows is replaced once it's let go, and the sharer's window shows each follower's state.

## 7. Settings, updates, and Linux

The Settings window, Velopack's update check, the app's own log, and the AppImage on the user's Steam Deck.

Exit: a Windows install from one release updates itself to the next, and the AppImage joins a session on the user's Steam Deck.

## 8. A session with friends

The user's group plays a session on Cyberpunk Co-op Tools instead of CP77 Log Collector.

Exit: the user reports a session where the host got everyone's logs and every follower got the host's builds.

## 9. Polish

A pixel pass on the real window against the frames, from snapshots.

Exit: the user calls the look done.
