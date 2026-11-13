# Collecting logs

```text
Purpose:     How coop-logs finds the logs on each player's PC, sends what's new to the host, and writes it into the host's session folder.
Depends on:  product/02-v1-scope.md, architecture/01-spine.md, network/01-relay-and-sessions.md
Decides:     What's picked up, how a file is recognized, what's sent and when, the messages, how resuming works, and the host's folder layout.
```

Every player's PC runs a `Sender`, which scans for logs, follows each file by its content, and hands the core messages for the host.
The host runs a `Collector`, which writes each player's messages into that player's folder inside the session folder, and answers each `Welcome` with what it already has, so the player goes on from where the host's copies end.
The behaviors are CP77 Log Collector's, read in its 1.3.1 source, and each difference from it is marked Changed or Added, or listed in [Left out](#left-out).

## The two halves

```text
 a player's PC                                   the host's PC
 ┌─────────────────────────┐   ToHost::Logs     ┌─────────────────────────────────┐
 │ Sender                  │   (one Batch each) │ Collector                       │
 │  scan, recognize, read  │ ─────────────────▶ │  <session>/<player>/game/...    │
 │  resume(Welcome.logs)   │ ◀───────────────── │  welcome() → Welcome.logs       │
 └─────────────────────────┘                    └─────────────────────────────────┘
```

The core calls both halves from one blocking thread, and neither half opens a socket or starts a thread:

| Call | When |
| --- | --- |
| `Sender::new` | at start, with the game folder, the game's `%LOCALAPPDATA%`, the app's log, and the extra paths the core knows |
| `Sender::resume` | with the `logs` of every `Welcome`, before the player's next tick; a `Sender` sends nothing before its first |
| `Sender::set_extra_paths` | when a `Welcome` brings extra paths other than the ones the `Sender` has |
| `Sender::tick` | every 250ms, sending each batch it returns as one `ToHost::Logs` |
| `Collector::new` | when the host starts a session, with the session folder |
| `Collector::welcome` | when a player says `Hello`, for their `Welcome.logs` |
| `Collector::on_message` | with each `ToHost::Logs` |
| `Collector::left` | when a player's socket closes |
| `Collector::tick` | at least every 2s, for `_status.txt` and for saving `_manifest.tsv` |

- The host runs a `Sender` for its own PC too, and makes the same calls with its own install ID on its own `Collector`, so the host's logs never cross the relay.
- The host never sends the `Sender` anything but `Welcome.logs`, so `Sender::on_message` ignores what it gets and `Collector::on_message` returns no replies.
  Both stay in the interface so the core's wiring holds if a later version needs replies.

## What's picked up

| Source | Where | What | Host's copies under |
| --- | --- | --- | --- |
| The game folder | the game folder, up to 40 folders deep | `*.log`, `*.log.N`, `*.log.txt`, and `*.txt` in folders named `logs` or `log` | `game/` |
| Crash reports | `REDEngine/ReportQueue` in the game's `%LOCALAPPDATA%` | what the game folder takes, and `*.txt`, `*.json`, `*.xml`, `*.ini` | `crash-reports/` |
| The game's own | `CD Projekt Red/Cyberpunk 2077` in the game's `%LOCALAPPDATA%` | what the game folder takes, and `*.json` | `localappdata/` |
| The app's log | the file the core names | that file | `app/` |
| Extra paths | the host's patterns, relative to the game folder | as the pattern says, below | `extra/` and the path in the game folder |

The filters and depths are CP77 Log Collector's (read in CP77 Log Collector 1.3.1, src/Scanner.cs).

- **Changed:** a file ending in `.dmp` is never sent, from any source, extra paths included.
  CP77 Log Collector sent crash dumps when a player ticked a box.
- **Changed:** the app's log is a source of its own, under `app/`, where CP77 Log Collector added it as one more extra path (read in CP77 Log Collector 1.3.1, src/Settings.cs).
- A folder that's a link, a symlink or a junction, isn't walked into.
- A folder holding `_collector.log` is skipped, since it's one of a host's session folders.
  **Changed:** CP77 Log Collector left out the host's output folder by its path (read in CP77 Log Collector 1.3.1, src/Settings.cs), and the session folder's marker covers the same case without the core passing the path in: a host whose logs folder sits inside the game folder doesn't send its own copies back to itself.
- **Changed:** a file two sources pick up goes once, under the first source in the table's order, and counts for both; CP77 Log Collector sent it under both paths.
- The size of each file comes from the file itself, since a folder listing can report a stale size for a file the game is still writing (read in CP77 Log Collector 1.3.1, src/Scanner.cs).

### Extra paths

**Changed:** the host lists the patterns, relative to the game folder, and every player's PC applies them to its own game folder, where CP77 Log Collector let each player list their own full paths (read in CP77 Log Collector 1.3.1, src/Settings.cs).
`bin/x64/StreetVendors/*.csv` picks up every `.csv` in that folder, and the host gets `extra/bin/x64/StreetVendors/vendors.csv` (a hypothetical file name).

- A pattern whose last part holds `*` or `?` takes the matching files in that folder, and none below it.
- A pattern naming a file takes that file.
- A pattern naming a folder takes its logs and `*.txt`, at any depth.
- **Changed:** a pattern with `..`, a leading `/`, a drive or any other `:`, or a wildcard before its last part, picks up nothing, so the host's patterns can't reach a file outside a player's game folder.
- **Changed:** a file an extra path picks up is sent even when it hasn't changed in 24 hours, since the host named it, while the 32 MB limit holds as for every file.
- Each pattern reports its own count, a refused one included, so the host sees a pattern that matches nothing.

## How a file is recognized

The `Sender` follows each file by its content, not its name, as CP77 Log Collector does (read in CP77 Log Collector 1.3.1, src/Sender.cs and src/Common.cs):

- **The prefix** is the file's first bytes, up to 512, and its FNV-1a 64-bit hash.
  When a followed file grows past its prefix, the prefix grows with it, up to 512 bytes, and the host is told with `Ident`.
- **The fingerprint** covers the bytes the host has, from where the host's copy starts in the source to where it ends: FNV-1a over all of them when there are 64 KB or less, or else over 32 samples of 64 bytes spread evenly, the last one ending at the end.
- A file the `Sender` follows only from its end, which the 24-hour rule below makes, has a fingerprint of the bytes before that end, so it's recognized before anything has been sent.
- A file on disk is the followed file when its first bytes hash to the prefix, it's at least as long as what was sent, and the same range of it has the same fingerprint.

A full scan matches every file it finds against the files the `Sender` follows, in this order:

1. A followed file still at its own path that matches stays where it is.
2. A followed file that doesn't match at its path is detached.
   A detached file that matches at its own path gets it back before anything is taken for a move, since two crash reports can begin with the same bytes.
3. Each file nobody claimed, in path order, that matches a detached file is that file under a new name.
   All such renames of one scan go to the host in one `Moves`.
4. A file nobody claimed that matches a file the host has data of is a copy, and goes as `Copy`.
5. Any other file is new, and goes as `Open` and then `Data`.
6. A detached file left over is gone, and goes as `Gone`, except at the first scan after a `Welcome`, which drops it without a word, since the host's list holds files the player deleted while away; either way the host keeps its copy.

So a mod's rotation goes like this:

```text
the player's PC              the host
x.log (A) → x.1.log (A)      renames its x.log to x.1.log, in one batch with any x.1 → x.2
new x.log (B)                opens a new x.log and receives B
```

A file that shrinks, or is rewritten with other first bytes or another fingerprint, no longer matches at its path: the `Sender` sends `Gone` and then the file as new, and the host moves its old copy into `_history`.

## What's sent and when

- **At the first scan after each `Welcome`,** a file the host has goes on from the end of the host's copy.
  By the 24-hour rule, a new file that hasn't changed in 24 hours is followed from its end, so only what it gains is sent.
  A new file over 32 MB sends its last 32 MB, from the first line start after the cut when one is within 64 KB of it (read in CP77 Log Collector 1.3.1, src/Sender.cs).
- **After that,** everything new is sent as it's written, and a new file a later scan finds sends its last 32 MB whatever its age.
- **A look** every 1s checks the size and modification time of each followed file, and reads the first bytes and fingerprint of each one that changed.
  An unchanged size and time count as an unchanged file until the next full scan, which reads every file's first bytes and fingerprint, since a file renamed into place can keep both.
- **A full scan** runs every 10s, and at once when a look finds a followed file missing, shorter than what was sent, or with other first bytes or another fingerprint.
  So a rotation, shrink, or delete is handled within 1s, and a new file within 10s.
- **Each tick** sends at most 1 MB of each file and 2 MB in all, in blocks of up to 256 KB, each compressed with deflate when that makes it smaller; the next tick starts at the file where the budget ran out, and looks at once while anything is left.
- **Changed:** CP77 Log Collector sent up to 4 MB of each file per round with no limit for the round, and its writes waited on TCP (read in CP77 Log Collector 1.3.1, src/Sender.cs); the core calls `tick` every 250ms with nothing to wait on, so each call has a bound of its own, 8 MB/s of logs in all.

## The messages

Every message is a Rust type in `coop-logs`, encoded with postcard.
A tick's messages go out in batches, and a batch closes before it passes 280 KB, so each `ToHost::Logs` stays under 300 KB unless one `Moves` is larger.
300 KB is our own bound, chosen to stay well under what a relay frame can hold (unverified: we haven't measured the largest frame the relay carries).

| Message | Fields | The host |
| --- | --- | --- |
| `Batch` | `generation`, then the messages | drops the batch unless the generation is the player's current one |
| `Open` | path, the offset the copy starts at, prefix length and hash, the source's size, modification time, and full path | moves any copy at the path into `_history`, and starts an empty one |
| `Data` | path, offset, raw length, whether it's compressed, the bytes | appends what it doesn't have yet |
| `Moves` | a list of from, to, and the source's full path | renames all of them at once, through `_tmp/` |
| `Copy` | from, to, and the source's full path | copies its copy |
| `Ident` | path, prefix length and hash | keeps the longer prefix |
| `Gone` | path | writes it in the player's events and keeps the copy |
| `Sources` | each source and how many files it picked up | keeps them for `Collector::sources` and `_peer.txt` |

`Welcome.logs` is the postcard encoding of `HostHas`, compressed with deflate: a generation, and for each copy its path, prefix length and hash, the offsets in the source where the copy starts and ends, and the copy's fingerprint.

- **Changed:** CP77 Log Collector's messages are binary frames of its own (read in CP77 Log Collector 1.3.1, src/Common.cs), and its welcome isn't compressed.
- **Added:** `Sources` carries the counts the player's window shows as the logs picked up on their PC, so the host's window can show them for every player.

## Resuming

The host sends a `Welcome` on a join, on a rejoin, and to every tester, a player other than the host, after its own socket came back.
Each carries `Collector::welcome`'s answer, and the core must pass it to `Sender::resume` before the player's next tick.

1. `Collector::welcome` saves the player's `_manifest.tsv`, reads it back, takes each copy's end from its size on disk rather than from the list, fingerprints each copy, and picks the player's next generation.
2. `Sender::resume` replaces what it follows with the host's list, each entry detached until the next full scan finds it.
   A file the host never got a byte of keeps its place, so what it gained meanwhile still goes out (read in CP77 Log Collector 1.3.1, src/Sender.cs).
3. The first scan matches the host's copies to the files on disk as any full scan does, so a rotation that happened while the player was away goes as `Moves`, and every copy goes on from its end.

**Added:** every batch carries the generation of the `Welcome` the `Sender` last resumed from, and the `Collector` drops any batch of another generation.
CP77 Log Collector has no generation, since every resume there came with a new TCP connection or relay stream.
Here a tester keeps its socket while the host's drops, and without one this happens:

1. The host's socket drops, and the relay drops what the tester sends meanwhile.
2. The host comes back, and sends the tester a `Welcome` with what it has.
3. A batch the tester sent before it got that `Welcome` reaches the host after the host sent it, and its `Data` lands past the end of the host's copy.
4. The host moves its copy into `_history` and starts a new one at that block's offset, and drops the tester's resent blocks as ones it already has, so the copy it goes on with lacks everything before that block.

With the generation, the host drops that late batch, and the tester sends everything after the host's ends exactly once.
The generation is the host's clock in nanoseconds, made larger than the last one, so a restarted host never hands out one a tester still holds.

A host that restarts on the same day keeps the session folder, `<YYYY-MM-DD> <code>`, reads each player's `_manifest.tsv` from it, and resumes them as before.
A host that starts on another day gets a new session folder, and every player sends again what the 24-hour rule and the 32 MB limit allow.

## Sources and progress

- The `Sender` reports how many files each source picked up after each full scan that changed the counts, and after each resume, so `Collector::sources` answers for each player.
  Before its first resume, the `Sender` still scans every 10s, so `Sender::sources` answers before the host does.
- `Sender::progress` is `CatchingUp { done, total }` while any file has more on disk than was sent after a tick: `total` counts every file that's had something left to send since the last time nothing was, and `done` the ones among them with nothing left.
  Otherwise it's `UpToDate`.
- `Sender::sent_bytes` and `Collector::received_bytes` count log bytes before compression, the first since the `Sender` started and the second since the `Collector` did.

## The host's folder

```text
<logs folder>/<YYYY-MM-DD> <code>/
  _status.txt          who is connected, files, data, and last data, every 5s
  _collector.log       everything that happened, every player
  <player>/
    game/...           the same layout as their game folder
    crash-reports/...
    localappdata/...
    app/...
    extra/...
    _peer.txt          their install ID, game folder, game version, system, sources, and connection state
    _events.log        new files, rotations, copies, files gone, and connections
    _manifest.tsv      which part of which source file each copy holds, and where it is here
    _history/<time>/   older copies a rotation, copy, or rewrite pushed out
    _long/             copies whose full path would be too long for Windows
```

The layout is CP77 Log Collector's, with `app/` added (read in CP77 Log Collector 1.3.1, README.md, "What the host gets", and src/Collector.cs).

- **A player's folder** takes the player's name, keeping letters, digits, `-`, `_`, and `.`, with `_` for anything else, and up to 40 characters.
  The folder belongs to the player's install ID for the whole session, found again through the `install id:` line of its `_peer.txt`, so a player whose Steam name changes keeps their folder.
- **Changed:** a name another player already has in the session gets `-2`, `-3`, and so on.
  CP77 Log Collector refused a second player of the same name while the first was connected, and otherwise wrote both into one folder (read in CP77 Log Collector 1.3.1, src/Collector.cs).
- **Paths from players** are cleaned before they touch the disk: in each part, control characters and `<>:"/\|?*` become `_`, and trailing dots and spaces go, so `..` becomes `_`; a Windows device name such as `CON` gets `_` in front; and a first part starting with `_` gets `x` in front, so no player can write over the host's own files.
- **`_long/`** holds a copy whose full path would pass 240 characters, as `<FNV-1a of the path, 16 hex digits>_<the file name's last 60 characters>`, and `_manifest.tsv` says which copy it is.
- **`_history/<YYYY-MM-DD_HH-mm-ss>/`** gets a copy that an `Open`, a move, or a `Copy` onto its path pushes out, or that a `Data` block found out of step with what the host had, under the copy's own path, with `.2`, `.3`, and so on added when that's taken.
- **`_manifest.tsv`** has a line per copy: the path, prefix length and hash, the offsets in the source where the copy starts and ends, the source's modification time and full path, and where the copy is in the player's folder.
  The `Collector` saves it at most every 2s while it changes, and whenever a player arrives or leaves or the `Collector` stops.
- **`_peer.txt`** is rewritten on every connection and disconnection, every change of sources, and every 5s.
- **`_events.log` and `_collector.log`** get a line per event, stamped with the host's local time, and `_collector.log` names the player on each line.
- **Copies** are plain files that other programs can read while they're being written, since Rust opens files with `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE` on Windows unless told otherwise ([`OpenOptionsExt::share_mode`](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html#tymethod.share_mode)).

## Left out

These parts of CP77 Log Collector have no counterpart in `coop-logs`:

- Each player's own settings for the hours and megabytes of history, crash dumps, and extra paths, which the v1 scope drops ([v1 scope](../product/02-v1-scope.md#whats-picked-up)), so the 24 hours and 32 MB are fixed.
- The `Info` and `Ping` messages, since the core pings the relay itself (read in this repository, src/core/src/session.rs, `PING_EVERY`).
- The remote address, tool version, and machine name in `_peer.txt`, since the relay hides addresses and `PeerInfo` carries only the game folder, the game version, and the system.
- The `Collector`'s copy of its events in the app's own log, since `coop-logs` has no handle on that log; the core can write what it wants there.

## Known limits

- A rotation that renames more than about 1,000 files in one scan makes a batch over 300 KB, since `Moves` has to arrive in one piece; it goes out as it is, and whether the relay carries it is unverified.
- A file rewritten with the same first bytes, at least as long as what was sent, and the same 32 samples of the sent range, is taken for the same file, so the host misses the rewrite (unverified: we haven't measured how often a log does that).
- Of a file over 32 MB that the host has no copy of, only the last 32 MB is sent, so a host that starts a new session folder loses the older part of such a file.
- `_history/` and the copies of files gone from a player's PC are never deleted, so a session folder only grows.
- The 24-hour rule compares a file's modification time to the player's clock, so a player whose clock is off by a day sends too much or too little history.
- The app's log is one file, so a rotated copy of it isn't picked up; TBD until stage 7 decides how the app's log rotates.
