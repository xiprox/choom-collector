# Build log

## 2026-11-13: An installed 0.2.0 found 0.3.0 by itself, built it from the delta, and restarted into it in 3s

We asked whether an installed copy updates itself from one release to the next, as [decision 004](../../decisions/004-releases-with-release-please-and-velopack.md) and stage 7's exit expect.
We expected the check to run 5s after start and fetch the 2.56 MB delta rather than the full package.

On the user's Windows 11 PC, at the user's request, 0.3.0 was uninstalled with `Update.exe uninstall --silent`, which left the data folder and its settings in place, and 0.2.0's `Setup.exe --silent` installed it (measured on 2026-11-13; the installer is unsigned, as every release is so far):

- `Setup.exe --silent` finished in about 1s and didn't start the app, so it was started through `Cyberpunk Co-op Tools.exe` in the install folder, as a shortcut does.
- 14s after it started, its log said "downloaded the update to 0.3.0": Velopack found 1 delta between 0.2.0 and 0.3.0, downloaded it in 4s, built the full 0.3.0 package from it in 2s, and deleted the 0.2.0 package and the delta.
- The title bar showed "Restart to update", and a click on it, posted by `tools/capture-window.ps1`, closed the window; 3s later the log said "Cyberpunk Co-op Tools 0.3.0 starting".
- 0.3.0 opened on the same session code with the same settings, its status bar said 0.3.0, the button was gone, and "Your mods" listed 50 mods from the user's real game, which 0.2.0, without mod sharing, couldn't.

This passes the first half of stage 7's exit; the Steam Deck half is the user's.

## 2026-11-13: 0.3.0 is the first release with delta packages, a quarter of the full one on Windows

We asked whether the release workflow makes delta packages once an earlier release has the same pack ID, which 0.1.0's `cp77-coop-tools` didn't give 0.2.0.
We expected a Windows delta well under the full package, since one exe changed.

The user merged the 0.3.0 release PR, and release `v0.3.0` came out of a 7m34s run with these packages (measured on GitHub Actions, 2026-11-13):

| Package | Delta from 0.2.0 | Full |
| --- | --- | --- |
| `win-x64` | 2.56 MB | 9.90 MB |
| `linux-x64` | 11.81 MB | 16.04 MB |

- The Linux delta is 74% of the full package, against 26% on Windows (unverified why: possibly the AppImage's compressed image differing throughout after a change).
- Whether an installed 0.2.0 applies the delta and starts 0.3.0 is the update test stage 7 still needs.

## 2026-11-13: The repository went public under MIT, and its releases read without a token

We asked whether an installed copy can find its updates once the repository is public, since Velopack's `GithubSource` reads the releases with no token, and a private repository's releases answer 404.
We expected the release list to read at once, and GitHub's license detection to lag behind.

The user chose to make the repository public under MIT, with the same license text as their other projects.
After the switch, the GitHub API listed `v0.1.0` and its 7 files to a request without a token, and 20s later GitHub reported the license as MIT (measured on 2026-11-13).

- Before going public, a scan of all 17 commits' files and messages for the user name, the machine name, the email, and home paths found only a made-up `C:/Users/*.txt` in a test, and the 10 pictures in the history show made-up sessions only.
- 0.1.0's pack ID is `cp77-coop-tools`, so a copy installed from it can't update to a `cyberpunk-coop-tools` release: stage 7's update test needs two releases under the new pack ID.

## 2026-11-13: The pipeline released 0.1.0 for Windows and Linux from a placeholder

We asked whether `.github/workflows/release.yml` builds, packs, and attaches both platforms when release-please's PR is merged, before any real app exists.
We expected the first run to fail somewhere in Velopack's packing, which we hadn't run on Linux.

A manual dry run packed both platforms first, and merging the 0.1.0 release PR then produced release `v0.1.0` with these files (measured on GitHub Actions, 2026-11-13):

| File | Size |
| --- | --- |
| `cp77-coop-tools-win-x64-Setup.exe` | 9.4 MB |
| `cp77-coop-tools-win-x64-Portable.zip` | 2.3 MB |
| `cp77-coop-tools-0.1.0-win-x64-full.nupkg` | 2.3 MB |
| `cp77-coop-tools-linux-x64.AppImage` | 4.9 MB |
| `cp77-coop-tools-0.1.0-linux-x64-full.nupkg` | 4.5 MB |
| `releases.win-x64.json`, `releases.linux-x64.json` | under 1 KB |

- The program was a placeholder of about 600 KB that printed its version, so the sizes are mostly Velopack's own.
- Velopack writes a non-default channel into every file name, which is what lets both platforms share one release.
- 0.1.0 went out under the pack ID `cp77-coop-tools`, before the rename to `cyberpunk-coop-tools`.
- Delta packages weren't made, since there was no earlier release; they're untested.

This backs [decision 004](../../decisions/004-releases-with-release-please-and-velopack.md).
