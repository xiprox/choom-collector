# Sharing mods

```text
Purpose:     How coop-mods finds a player's mods, shares them, and keeps each follower's copy the same as the sharer's.
Depends on:  product/02-v1-scope.md, architecture/01-spine.md, network/01-relay-and-sessions.md
Decides:     How mods are found and grouped, the messages and who sends each, how a follow is applied, held files
             and game restarts, unfollowing, the hub's cache, delivery state, flow control, and what
             survives a restart.
```

`coop-mods` holds two objects that the core drives from one thread: a `Player` on every PC, the host's included, and a `Hub` on the host's PC.
A sharer's `Player` lists its shared folders to the hub, the hub lists every shared folder to every player, a follower's `Player` asks the hub for each file its copy lacks, and the hub pulls each file from its sharer once and passes it on to everyone who asked.
The crate never touches the network: the core carries its messages as opaque bytes in `ToHost::Mods` and `ToTester::Mods`, and passes them between the host's own `Player` and the `Hub` in-process.
Every behavior marked as read here comes from CP77 Log Collector at commit 5cbb774, most of it from `src/Mods.cs`, and each change from that app is marked as one.

## What the core does

- Calls `Player::tick` every 250ms, and `Hub::tick` as often, from the thread that runs the session.
- Calls `connected(true)` on a new `Player`, the host's own included, before it sends anything, and `connected` again whenever the player's link to the hub drops or comes back.
- Calls `Hub::joined` when a player's link opens, the host's own `Player` included, and `Hub::left` when the player's socket closes, not when the session forgets them, or a dropped follower shows as paused 60s late.
- Delivers `Target::Everyone` and `Target::Player` to the host's own `Player` too, in-process.

## Finding mods

A mod is every top-level folder with one name, capitals aside, in the game's seven mod places, which [the v1 scope](../product/02-v1-scope.md#finding-mods) lists; its key is that name in lower case (read in CP77 Log Collector, src/Mods.cs, `ModLayout.Places`, and src/MainWindow.cs, `MergeMods`).
So `r6/scripts/EquipmentEx` and `archive/pc/mod/equipmentex` are one mod, `equipmentex`.

- A folder's files are found by walking it 24 levels deep, through links and junctions, since a modder often links their working copy into the game folder (read in CP77 Log Collector, src/Mods.cs, `ModLayout.Walk`).
- A file or folder that a pattern matches stays out of everything: it's never listed, sent, compared, or deleted.
  The patterns are the scope's defaults, the app's own `*.coop-tmp`, and the sharer's own patterns from `PlayerConfig::never_share`.
  `*` and `?` match within a name, capitals aside, and a pattern ending in `/` matches a folder at any depth (read in CP77 Log Collector, src/Mods.cs, `ModLayout.Excluder`).
- A loose file belongs to a mod when its name, after any `!`, `#`, `-`, `_`, or space put in front to set the load order, starts with the mod's name and the next character isn't a letter or digit: `archive/pc/mod/#EquipmentEx-patch.archive` belongs to `EquipmentEx` (read in CP77 Log Collector, src/Mods.cs, `ModLayout.LooseBelongs`).
  `MyMod::loose_file` names the first one.
- `Player::my_mods` comes from a walk every 10s, with sizes and no hashes.

## Sharing

- **Changed:** `share(key)` shares every folder of that name in any place, folders made later included.
  CP77 Log Collector shares folder by folder, and its row offers Share again for a folder added later (read in CP77 Log Collector, src/MainWindow.cs, `OnModShareClicked`).
  Sharing by name half-ticks a follower's row, some of its folders followed and some not, as soon as a sharer creates a new folder for the mod, which is how a modder adds a TweakXL or archive part to a mod in progress.
- Sharing a mod stops following it, and following a mod stops sharing it, since a folder can't be a copy and the original at once (read in CP77 Log Collector, src/MainWindow.cs, `Follow` and `OnModShareClicked`).
- The sharer walks its shared folders every 2s, hashes each file whose size or modified time changed, and sends `Share` when the list differs from the last one sent (read in CP77 Log Collector, src/Mods.cs, `ModSharer.Scan`).
- A file that changed and isn't hashed yet keeps its last entry, so a file being saved never looks deleted (read in CP77 Log Collector, src/Mods.cs, `ModSharer.Scan`).
- **Added:** a folder with a new file that isn't hashed yet goes out as it last went, or not at all until it's hashed, and a sharer's first `Share` after the app starts waits until every shared file is hashed.
  CP77 Log Collector hashes as it scans, but `coop-mods` spreads hashing over ticks, and a list with files missing would make every follower delete them.
- Of two files in one folder whose paths differ only in capitals, the first in ordinal order is shared, since they'd be one file on a follower's Windows PC (read in CP77 Log Collector, src/Mods.cs, `ModSharer.Scan`).
- **Added:** each shared folder carries the time it last changed, for the window's Changed column.
  The first walk takes its newest file's modified time, and each later change in its list takes the time of that walk.

### One sharer per folder

The hub gives each folder, by path with capitals aside, to the first player to offer it, or, of several offering it at once, the one who joined first.
That player keeps it until they stop sharing it, and then it goes to the earliest-joined player still offering it (read in CP77 Log Collector, src/Mods.cs, `ModHub.Rebuild`).
A mod whose folders two players offered can be split between them, and then it's two rows, one per sharer, since a follower trusts each of them apart (read in CP77 Log Collector, src/MainWindow.cs, `MergeMods`).

- The list names each sharer's PC by its PC ID, the first 8 bytes of the SHA-256 of its install ID in hex, because the host knows a returning tester by the install ID itself (read in CP77 Log Collector, src/Mods.cs, `ModLayout.PcId`).
- **Changed:** when a player's link drops, the hub keeps their folders for 60s, `AWAY_GRACE` in `hub.rs`, which matches `TESTER_GRACE` in `src/core/src/session.rs`.
  The list marks those folders away, and followers show the mod as paused.
  CP77 Log Collector hands a folder over the moment its sharer's connection drops (read in CP77 Log Collector, src/Collector.cs, `PeerGone`), but Choom Collector reconnects by itself ([relay and sessions](../network/01-relay-and-sessions.md#reconnecting)), and a handover on every drop would make each follower agree to the sharer again.

## Messages

`ToHub` and `FromHub` in `src/mods/src/wire.rs` are the messages, encoded with postcard.
The first five are CP77 Log Collector's MODSHARE, MODLIST, MODWANT, MODPULL, and MODDATA (read in CP77 Log Collector, README.md, "Protocol", and src/Common.cs, `Msg`), and the rest are new.

| Message | From, to | Carries | Sent |
| --- | --- | --- | --- |
| `Share` | player to hub | the sharer's own patterns, and each shared folder with its changed time and its files' paths, sizes, and SHA-256 | when the list changes, and on every new link |
| `List` | hub to every player | every shared folder with its sharer's name and PC ID, their patterns, whether they're away, its changed time, and its files | when it changes, and to a player who joins |
| `Want` | player to hub | each file its followed folders lack, with how many bytes of it the player has | when a file is added to it or dropped before it arrived, after 20s with no data, and after a block arrives out of order or a file fails its hash |
| `Pull` | hub to a sharer | the files the hub needs from them, with how many bytes it has, replacing the last `Pull` | when wants change, and again after 20s with no upload |
| `Data` | sharer to hub, hub to follower | up to 256 KiB of one file, with its path, hash, size, and offset | as the receiver's window allows |
| `Ack` | both ways | the bytes of `Data` taken in since the link opened | one for each `Data` |
| `Deliveries` | player to hub | the player's delivery for each folder they follow | when it changes |
| `Followers` | hub to a sharer | each follower's delivery for each folder the sharer shares | when it changes |
| `Piece` | both ways | a part of a message over 300 KiB | in place of that message |

The hub keeps a list of what each player wants, and sends a player's file from its cache whenever the cache holds the next bytes of it, even while the sharer's upload is still arriving (read in CP77 Log Collector, src/Mods.cs, `ModHub.SendSome`).

### Flow control and message size

- **Added:** each side sends `Data` until 2 MiB of it is unacknowledged, and the receiver's `Ack` opens the window again.
  CP77 Log Collector writes each block to a socket, and a full socket makes the next write wait (read in CP77 Log Collector, src/Sender.cs, the sending loop).
  `coop-mods` hands its messages to the core and never waits, so without the window a 2 GiB archive would sit in memory on its way to the relay.
- **Changed:** the follower sends no new `Want` when a file finishes arriving, since the hub drops a file from its queue once it has sent all of it.
  CP77 Log Collector sends one whenever its list of files changes (read in CP77 Log Collector, src/Mods.cs, `ModClient.Sync`), and the hub, taking the offsets in it, sends again the bytes of the next file that were already on their way.
- **Added:** a message over 300 KiB, such as the list of a mod with thousands of files, goes in `Piece`s of 256 KiB, which the receiver joins before reading.
  `Data` never needs pieces, as its 256 KiB of file data and its header stay under 300 KiB.
- **Changed:** blocks travel uncompressed, where CP77 Log Collector deflates each one (read in CP77 Log Collector, src/Mods.cs, `ModBlock.Build`).
  Most of a mod's bytes are in `.archive` files, which hold compressed data already (unverified: we haven't measured what deflate saves on them).

## Following

A follow is a folder path, the PC ID of the sharer the player agreed to, and that sharer's name, so when another PC takes the folder over, nothing changes until the player ticks it again (read in CP77 Log Collector, src/Mods.cs, `ModFollow`).

`Player::shared` builds one row per mod and sharer from the hub's list and the player's follows.

- A row's key is the mod's key, and when two PCs share parts of one mod, each of its rows adds `|` and the sharer's PC ID, which no folder name can hold.
- A row's name is spelled as its folder in the first of the mod places in the scope's order, as `my_mods` spells it.
- The half tick (`Follow::AddedFolder`) is a row with some of its folders followed and some not, which is how a folder the sharer added shows (read in CP77 Log Collector, src/MainWindow.cs, `MergeMods`).
  Nothing arrives in the new folder until `take_added_folder`, which follows the rest of the row.
- A follow whose folder the list no longer has keeps its row with the status `NotShared`, and the copy stays as it is (read in CP77 Log Collector, src/Mods.cs, `ModClient.Views`).
- A row from a PC other than the one a follow names shows `FollowedElsewhere`, with the name of the sharer the player agreed to.

`ModStatus` holds these states for a row:

| Status | When |
| --- | --- |
| `Sharing` | the player shares it |
| `Paused` | the player's link to the host is down, or the sharer's |
| `FollowedElsewhere { by }` | the player follows these folders from `by`, and another PC shares them now |
| `OfferedToo` | the player offered their own copy, which goes out once this sharer stops |
| `NotOnYourPc` | not followed, and no copy here |
| `OnYourPc { same }` | not followed, with a copy here; `same` says whether it matches the sharer's |
| `AddedFolder` | the half tick |
| `NotShared` | followed, and nobody lists its folders |
| `Fetching { done, total }`, `NeedsRestart`, `UpToDate` | followed, as [delivery state](#delivery-state) says |
| `Checking` | followed, and the copy here is still being hashed |

### Applying a follow

Every second, and at once after a new list or a finished file, the follower brings each followed folder in line with the sharer's list (read in CP77 Log Collector, src/Mods.cs, `ModClient.Mirror`):

1. Each local file the sharer's list lacks, apart from what the patterns leave out, is deleted.
2. Each of the sharer's files whose local copy differs in size or SHA-256 goes into the next `Want`.
3. **Changed:** its `Data` goes into `<file>.coop-tmp` beside the file it replaces, and the follower hashes the bytes as they arrive.
4. Once the file is complete and its hash matches the list, a rename puts it in place of the old one, so the game never reads half a file.

Step 1 removes a file added in a followed folder, and steps 2 to 4 fetch a changed one again, so an edit there is undone within 3s (measured on Windows 11, 2026-11-13, [log](log.md#2026-11-13-players-and-a-hub-in-one-process-keep-a-followers-copy-exact-and-a-held-file-lands-once-its-let-go)).
CP77 Log Collector writes a file on its way to a folder in its data folder and copies it beside the target once complete (read in CP77 Log Collector, src/Mods.cs, `ModClient.OnData` and `ModClient.Install`), but copying a large archive from another drive would hold up a tick, and a rename within one folder takes no time.

- A file of 0 bytes is written in place without fetching it.
- A follower takes `Data` only for a file in its current `Want`, with the path and hash it asked for, so the hub can't write anywhere else.
- Followers check every path in the list again before writing anything, and drop any file outside its folder (read in CP77 Log Collector, src/Mods.cs, `ModClient.OnList`).
- On Linux, a folder or file that exists with other capitals is used as it is (read in CP77 Log Collector, src/Mods.cs, `ModLayout.Local`), since we expect the game, through Proton, to find files whatever their capitals (unverified: starting the game with a followed folder in other capitals settles it).
- A `Player` that's dropped deletes the `*.coop-tmp` files it was writing, and on its first tick a `Player` deletes any left in its followed folders by a run that crashed.

### Path checks

The hub takes a folder only when its path is a mod place followed by one name, and a file only inside that folder; a follower checks the same before it writes (read in CP77 Log Collector, src/Mods.cs, `ModLayout.FolderOf`, `ModLayout.ReadFiles`, and `ModHub.OnShare`).
A path fails the check when it's empty or over 400 characters, holds `\` or `:`, or has a part that's empty, `.`, `..`, ends in a dot or a space, or holds a control character or one of `<>"|?*`.
So `/etc/passwd`, `C:/Windows`, and `r6/scripts/../../bin` all fail.
The hub takes a file's `Data` only from the folder's sharer, with the size and hash they listed (read in CP77 Log Collector, src/Mods.cs, `ModHub.OnUpload`).

## Held files and game restarts

While the game runs on Windows, it holds the files it loaded without letting others delete them, and Windows refuses both a rename over such a file and its deletion.
The follower keeps the finished file beside its target and tries again on every pass, so it lands once the game lets go (read in CP77 Log Collector, README.md, "Sharing mods", and src/Mods.cs, `ModClient.Install`).
On Linux the rename succeeds, and the game keeps reading the old file it has open (read in CP77 Log Collector, src/Mods.cs, `ModClient.Install`).

The game loads mods only when it starts, so a follower **needs a game restart** for a folder when a change landed in it while the game ran, or a file in it waits for the game to let go.
The mark clears when the follower sees the game isn't running.

- **Added:** the follower tells the game runs by looking for a process named `Cyberpunk2077.exe` every 2s: in Windows' process list, and on Linux in each process's first argument in `/proc/<pid>/cmdline`, or its name in `/proc/<pid>/comm`, which the kernel cuts to `Cyberpunk2077.e`.
  We chose the process over opening `bin/x64/Cyberpunk2077.exe` for writing: that open fails while the game runs on Windows, but under Proton the kernel doesn't load the `.exe` as a program, so it doesn't refuse the open (unverified), and the open fails anyway in a game folder the player can't write to.
- We haven't checked that the game's process under Proton shows `Cyberpunk2077.exe` either way; reading `/proc` while it runs settles it.
- A held file while the game isn't running, held by an editor, say, counts as a file still fetching.

## Unfollowing

`unfollow(key, delete_copy)` stops following the row's folders, and drops their files on their way.
With `delete_copy`, it deletes each followed folder that's on the PC (read in CP77 Log Collector, src/Mods.cs, `ModLayout.DeleteFolder`).

- A folder that's a link or junction to one elsewhere loses only the link, so a modder's working copy is never touched (read in CP77 Log Collector, src/Mods.cs, `ModLayout.DeleteTree`).
- When files are left, such as those a running game holds on Windows, it returns an error naming how many, and calling it again tries again.
- **Changed:** it deletes only the folders the player followed.
  CP77 Log Collector deletes every folder of the row that's on the PC (read in CP77 Log Collector, src/MainWindow.cs, `AskToDeleteCopy`), so a folder the sharer added and the player never agreed to, which may be the player's own, would go too.

## No backups

- **Dropped:** CP77 Log Collector's backups of what following replaces or deletes, and what unfollowing deletes (read in CP77 Log Collector, src/Mods.cs, `ModBackups`).
  The user dropped them in the polish pass (2026-11-13): a follower's copy is the sharer's, which the sharer still has, and a follower isn't meant to edit it.

## The hub's cache

The hub stores each file it pulls in `mod-cache/` in the host's data folder, named by its SHA-256 ([the spine](../architecture/01-spine.md#files-on-disk)), so a file goes up from its sharer once, however many players want it (read in CP77 Log Collector, src/Mods.cs, `ModHub`).

- The hub hashes each upload as its blocks arrive, and a complete file whose hash differs from the list is thrown away and pulled again, since the sharer changed it on the way.
- A file stays for 10 minutes after it was last read or written, checked every 60s, and the cache empties when a `Hub` is made and when it's dropped (read in CP77 Log Collector, src/Mods.cs, `ModHub.Sweep`, `Start`, and `Stop`).
- A file on its way picks up where it stopped: the hub keeps what arrived when its sharer drops, and asks from there once they're back, and a follower keeps its `.coop-tmp` across a drop of its own link and asks from there.
  A follower that restarts starts the file over, as CP77 Log Collector does (read in CP77 Log Collector, src/Mods.cs, `ModClient.Start`).

## Delivery state

**Added:** each follower reports its delivery for each followed folder, and the hub passes it to the folder's sharer, so the sharer's window shows each follower's state.

| Delivery | When |
| --- | --- |
| `UpToDate` | the folder matches the sharer's list |
| `Fetching { done, total }` | files are missing, where `total` counts the files needed since the folder was last up to date |
| `NeedsRestart` | as [held files and game restarts](#held-files-and-game-restarts) says |
| `Paused` | the hub sets it for a follower whose link is down |

- A folder still being hashed reports nothing new, and a follower who hasn't reported a folder yet isn't listed for it.
- `Player::doing` sums `Fetching` across every follow, or else says `NeedsRestart` when any follow needs it, for the player's line in the players list.

## What survives a restart

`mods.toml` in the data folder holds the keys the player shares and their follows, written after each change.
Files on their way, the hub's cache, and a deletion waiting for a retry don't survive.

## Bounded work

A tick runs on the thread that runs the session, so it does a bounded amount of work.
Hashing reads at most 32 MiB a tick, picking up a large file where the last tick stopped, and the sharer's files and followed files go before the comparisons the window shows for mods the player doesn't follow, which run every 5s.

## Known limits

- `unfollow` with `delete_copy` deletes and backs up the whole mod before it returns.
- **Dropped:** following a folder by name whoever shares it, which CP77 Log Collector offers through `--subscribe` (read in CP77 Log Collector, src/Mods.cs, `ModFollow`), since Choom Collector has no such option.
- A game that closes and starts again within the 2s between checks keeps a stale restart mark until it closes.
- `Fetching` counts files, so a follower fetching one large archive shows 0 of 1 until it lands.
- A follower can't hash a file the running game holds without letting others read it, and shows the mod as checking until the game closes (unverified: we don't know whether the game holds any file that way).
