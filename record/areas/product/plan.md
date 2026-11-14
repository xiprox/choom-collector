# Plan

The plan builds v1, as [the v1 scope](02-v1-scope.md) describes it, in stages that each end in something we can watch work.
Each stage writes the design docs it needs before its code, and the polish pass comes last, at the user's request.
A stage is done when a commit meets its exit criterion and adds a "Done on" line under it.

## 1. Record and workspace

The record, the Cargo workspace with the crates [the spine](../architecture/01-spine.md#crates) lists, and CI building the window on both platforms.

Exit: CI builds Windows and Linux from `main`, and the program opens an empty window on Windows.

Done on 2026-11-13: CI built both platforms for commit 8da57bc, gpui's Linux packages included, and `tools/capture-window.ps1` pictured the empty window.

## 2. The window, on made-up data

The theme and every screen of [the interface decisions](../interface/decisions.md), fed by `--demo`, a made-up session that shows the states the frames show, and `--data-dir <folder>`.

Exit: pictures from `tools/capture-window.ps1` of setup for joining and for hosting, and of the window as the host and as a tester, show every element the frames in `experiments/areas/interface/frames/` show.

Done on 2026-11-13: the pictures are in `experiments/areas/interface/stage-2/` ([interface log](../interface/log.md#2026-11-13-the-window-draws-every-frame-in-gpui-from-made-up-state)).

## 3. The relay

`relay/`, forked from fsc-editor's, carrying binary messages, with a deploy guide, and the relay client in `coop-core`.

Exit: under `wrangler dev`, a test's host and two testers exchange messages, the host hears a tester leave, and a host that drops and returns within 30s keeps its testers.

Done on 2026-11-13: `src/core/tests/relay.rs` passes against the fork ([network log](../network/log.md#2026-11-13-the-forked-relay-carries-binary-frames-between-a-host-and-two-testers-and-keeps-the-testers-through-a-hosts-drop)).

## 4. Sessions

Settings, the first run, hosting and joining on start, players and their state, and reconnecting, with the window on real state.

Exit: on one PC, a host and two testers, each with their own data folder, see each other in the window, and a tester that restarts rejoins with no action.

Done on 2026-11-13 ([network log](../network/log.md#2026-11-13-a-host-and-two-testers-on-one-pc-find-each-other-and-a-restarted-tester-rejoins-in-its-own-place)).

## 5. Logs

Collection as the scope describes it: sources, history, following by content, rotations, resuming, the host's folder, which logs each player's PC picks up, and extra paths from the host.

Exit: in a test session, a line a tester's log gains is in the host's copy within 2s, a rotation renames the host's copies without sending them again, and a tester that restarts sends nothing twice.

Done on 2026-11-13 ([logs log](../logs/log.md#2026-11-13-in-a-real-session-a-testers-new-line-reached-the-host-in-096s-a-rotation-was-a-rename-and-a-restart-sent-nothing-twice)).

## 6. Mods

Sharing as the scope describes it: finding and merging, sharing and following, the host's cache, applying, held files on Windows, the DLL warning, loose files, unfollowing, and each follower's state for the sharer.

Exit: in a test session, a file the sharer saves is in a follower's copy within 5s, a file a running program holds on Windows is replaced once it's let go, and the sharer's window shows each follower's state.

Done on 2026-11-13 ([mods log](../mods/log.md#2026-11-13-in-a-real-session-a-saved-file-reached-the-follower-in-205s-and-a-held-one-landed-034s-after-it-was-let-go)).

## 7. Settings and updates

The Settings window, Velopack's update check, and the app's own log.

Exit: a Windows install from one release updates itself to the next.

- **Dropped:** an AppImage joining a session on a Steam Deck, which this plan first asked for from CP77 Log Collector's support for Steam Deck desktop mode (read in CP77 Log Collector, README.md), until the user said nobody in their group plays on Linux or a Deck (2026-11-13).
  The Linux build stays; [the v1 scope](02-v1-scope.md#updates-and-platforms) says what it has run on.

Done on 2026-11-13 ([build log](../build/log.md#2026-11-13-an-installed-020-found-030-by-itself-built-it-from-the-delta-and-restarted-into-it-in-3s)).

## 8. A session with friends

The user's group plays a session on Choom Collector instead of CP77 Log Collector.

Exit: the user reports a session where the host got everyone's logs and every follower got the host's builds.

## 9. Polish

A pixel pass on the real window against the frames, from snapshots.

Exit: the user calls the look done.
