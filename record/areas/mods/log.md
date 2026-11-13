# Mods log

## 2026-11-13: Players and a hub in one process keep a follower's copy exact, and a held file lands once it's let go

We asked whether `coop-mods`, as [sharing mods](01-sharing.md) describes it, ports CP77 Log Collector's sharing and following to a hub and players that pass opaque messages, with no network and no threads.
We expected the moving parts CP77 Log Collector got from its threads and sockets to need replacing, and they did: hashing spread over ticks, a window of acknowledged data where a blocked socket write used to pace the sender, and messages split under 300 KiB.

`cargo test -p coop-mods` ran 34 tests, and all passed in under 10s together (measured on Windows 11, 2026-11-13).
The 24 in `src/mods/tests/` run a hub and two or three players in one process, each with a game folder and a data folder of its own, on a clock the test moves 250ms a step, and fail any message over 300 KiB:

- A mod's folders of one name in two places make one row, its loose file is found, files the defaults match stay out of its size, and a `.dll` sets `has_dll`.
- A follower's copy becomes the sharer's, files of 0 bytes and 700 KB included, while her own log stays and what was replaced goes to her backups.
- A changed, a new, and a deleted file, and a deleted subfolder, reach the follower within 5s of steps.
- An edit in a followed folder is undone within 3s.
- A folder the sharer adds half-ticks the follow, and nothing arrives until `take_added_folder`.
- Each follower's delivery reaches the sharer: up to date, needs a restart while the game probe says the game runs, paused while the follower's link is down.
- A follower that takes one message a step shows fetching 0 of 4 files, then 1, 2, or 3 of 4, with the same count on its own row, in `doing`, and on the sharer's row.
- Unfollowing with delete removes both folders after backing them up, CET's `db.sqlite3` included; a folder that's a junction made with `mklink /J` loses only the junction, and what it pointed at stays.
- The sharer's own patterns and the defaults keep files out on both sides.
- A restart of sharer and follower keeps the share and the follow, and the next change still arrives.
- When the first sharer stops, the second takes the folder, and the follower's copy stays as it was until the follower ticks again.
- A sharer whose link drops after two blocks of a 1.5 MB file sends only the rest once back: the hub took exactly 1,500,000 bytes from them in all.
- A sharer gone past the 60s grace leaves the follower's copy as it was, shown as not shared.
- The hub's cache keeps a file past 9 minutes unused, has none after 11, and is gone when the hub is dropped.
- A mod of 2,500 files arrives whole, its list and wants going in pieces.
- A follower whose folder exists as `mymod` gets `MyMod`'s files in it, with no second folder.
- The hub drops a shared folder or file outside a mod place, `..`, `/etc`, and `C:/Windows` included, and data from anyone but the sharer; a follower given a forged list and data writes only the one file it asked for, with the hash it asked for.
- With a 120-byte cap and a 100-byte file limit, the backups drop the oldest folder and skip a 200-byte file.

On Windows, a file the test held open with `FILE_SHARE_READ` alone, standing in for the running game (unverified: we haven't checked the game's share mode), made `std::fs::rename` over it fail; the new file waited beside it as `held.archive.coop-tmp`, the sharer's row showed `NeedsRestart`, and the file landed within 3s of the handle closing (measured on Windows 11, 2026-11-13, `a_file_the_game_holds_is_replaced_once_it_lets_go`).
Rust's `std` reports a junction as a link, so `delete_folder` removes only the junction and never walks into it (measured in the same run, `unfollowing_a_linked_folder_deletes_only_the_link`).
`cargo clippy --all-targets` passes for `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`, but the Linux build is only checked, not run, so `game::running` on Linux and `layout::local` with real capitals are untested there.

Writing the first `Share` showed a trap CP77 Log Collector doesn't have: hashing spread over ticks makes the first scan after a start list every file as unknown, and a `Share` sent then would have told every follower to delete their copies.
A sharer's first `Share` now waits until every shared file is hashed, and a folder with an unhashed new file goes out as it last went.

What changed from CP77 Log Collector is marked as changed, added, or dropped in [sharing mods](01-sharing.md), with its reason.

This affects [sharing mods](01-sharing.md), which this entry's tests check.
Outside `src/mods/` and `record/areas/mods/`, `record/index.md` doesn't list the mods area yet, the spine's files on disk lack `mods.toml`, `ModStatus` in `src/core/src/state.rs` lacks `Checking`, `OnYourPc`, `FollowedElsewhere`, `NotShared`, and `OfferedToo`, and `TESTER_GRACE` in `src/core/src/session.rs` should say that `AWAY_GRACE` in `src/mods/src/hub.rs` matches it.
