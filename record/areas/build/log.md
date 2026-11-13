# Build log

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
