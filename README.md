# Cyberpunk Co-op Tools

Play Cyberpunk 2077 together: everyone's logs in one place, and mods shared between friends.
A Rust rewrite of CP77 Log Collector, at its very start: the app itself is a placeholder for now.

## Releases

Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat: …`,
`fix: …`, `chore: …`), as release-please reads them:

1. Every push to `main` updates release-please's release PR, with the next version and the
   changelog. Before 1.0, a `feat` raises the minor version and a `fix` the patch.
2. Merging that PR creates the GitHub release and its tag.
3. The same workflow then builds Windows and Linux, packs them with
   [Velopack](https://velopack.io), and attaches them to the release: a Windows installer
   (`cyberpunk-coop-tools-win-x64-Setup.exe`), a portable zip, a Linux AppImage, and the packages the
   app updates itself from, deltas included.

Windows and Linux use the `win-x64` and `linux-x64` channels, so their files never clash on
the shared release.

To try the packaging without releasing anything: Actions → Release → Run workflow. The
packages are kept as that run's artifacts.

CI builds and tests both platforms on every push.
