# Releasing

A release is one merge: release-please keeps a release PR open on `main`, and merging it starts the Release workflow, which builds, packs, and publishes the release.
[Decision 004](../../decisions/004-releases-with-release-please-and-velopack.md) says why, and `.github/workflows/release.yml` is the workflow.

## How a version is chosen

release-please reads each commit title on `main` since the last release, so titles follow Conventional Commits, as [writing.md](../../../.claude/rules/writing.md#commit-messages) describes.
Before 1.0, a `feat` or a breaking change (`!`) raises the minor version, and a `fix` raises the patch; other types don't make a release on their own.
The title becomes the changelog line, in `CHANGELOG.md`.

## How to make a release

1. If `main` has a `feat` or `fix` commit since the last release, push it.
   You'll see release-please open or update its release PR, with the next version and the changelog.
   With neither, its log says "No user facing commits found" and it skips the PR (measured on the run for 2ef8f66, 2026-11-14).
2. Merge the release PR.
   You'll see a GitHub release and its tag appear.
3. Wait for the Release workflow's two `package` jobs, one per platform, to finish.
   You'll see these files on the release, where `<channel>` is `win-x64` or `linux-x64`:

   | File | What it is |
   | --- | --- |
   | `choom-collector-win-x64-Setup.exe` | the Windows installer |
   | `choom-collector-win-x64-Portable.zip` | the Windows app, run without installing |
   | `choom-collector-linux-x64.AppImage` | the Linux app |
   | `choom-collector-<version>-<channel>-full.nupkg`, and `-delta.nupkg` unless the release before it went out under another pack ID | the packages installed copies update from |
   | `releases.win-x64.json`, `releases.linux-x64.json` | the update feeds installed copies read |

## How to try the packaging without releasing

1. In GitHub, open Actions, then Release, then Run workflow.
   You'll see a new Release run start, with two `package` jobs.
2. When the run finishes, download its `packages-win-x64` and `packages-linux-x64` artifacts.
   They hold a release's files, versioned `<version>-dryrun.<run number>` and without delta packages, and nothing is published.
