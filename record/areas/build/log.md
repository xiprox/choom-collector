# Build log

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
