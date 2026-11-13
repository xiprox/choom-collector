# 004: Releases come from release-please, packed with Velopack

A push to `main` updates release-please's release PR, merging it creates the GitHub release, and the same workflow builds Windows and Linux, packs each with Velopack, and attaches the installers and update packages to the release.
Decided by the user on 2026-11-13, who asked for "release please that builds on push and creates a github release + velopack chunked updates".

## Context

CP77 Log Collector is built by a script on the user's PC and shared as a link, so every friend downloads 21 MB again for each fix (read in CP77 Log Collector, README.md).
An app for anyone needs releases that update themselves, built where anyone can see how.

## Options

1. **Build locally and upload by hand.**
   Nothing to set up, but every release is a person's afternoon, and every player downloads the whole app again.
2. **Tag releases by hand, and let CI build and attach them.**
   The builds are reproducible, but versions and changelogs are still written by hand, and nothing updates itself.
3. **release-please, with Velopack's packing and delta packages in the same workflow.**
   Versions and changelogs come from commit titles, and installs update themselves, at the cost of a commit format everyone follows.

## Decision

Option 3, built in `.github/workflows/release.yml`.
It released 0.1.0 with a Windows installer of 9.4 MB, a portable zip, a Linux AppImage of 4.9 MB, and both update feeds, from a placeholder program ([build log](../areas/build/log.md#2026-11-13-the-pipeline-released-010-for-windows-and-linux-from-a-placeholder)).

## What it commits us to

- Commit titles follow Conventional Commits ([writing.md](../../.claude/rules/writing.md#commit-messages)).
- Windows packs on the `win-x64` channel and Linux on `linux-x64`, so their files never clash on the shared release.
- The `velopack` crate's version and the workflow's `VPK_VERSION` move together.
- Velopack's pack ID is `choom-collector`; 0.1.0 went out as `cp77-coop-tools`, so the workflow skips it as a base for delta packages.
- Delta packages are unverified until a release follows one made under the same pack ID.
- The app reads its updates from the repository's GitHub releases, which a private repository hides from anyone without a token (unverified), so updates wait for the repository to go public.
