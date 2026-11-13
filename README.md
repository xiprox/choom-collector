# Choom Collector

Play Cyberpunk 2077 together: everyone's logs in one place, and mods shared between friends.
A Rust rewrite of CP77 Log Collector, for groups that test mods together.

- **Logs:** every player's game, mod, and crash logs go to the host as they're written, into a
  folder per session and per player, ready to hand to Claude or anyone else to read.
- **Mods:** whoever builds a mod shares it with a tick, and the friends who follow it get each
  build in their game folder.

The host and the testers run the same app. Everyone connects through a relay the host runs on
Cloudflare's free plan, so nobody opens a port; [relay/README.md](relay/README.md) shows how to
set one up.

It's for Cyberpunk 2077 only. A fork for another game is welcome.

## Status

Before 1.0, and being built: logs work end to end, mod sharing is on its way, and the look gets
its last pass at the end. [record/areas/product/plan.md](record/areas/product/plan.md) has the
stages and where each one stands.

## Releases

Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat: …`,
`fix: …`, `chore: …`), as release-please reads them:

1. Every push to `main` updates release-please's release PR, with the next version and the
   changelog. Before 1.0, a `feat` raises the minor version and a `fix` the patch.
2. Merging that PR creates the GitHub release and its tag.
3. The same workflow then builds Windows and Linux, packs them with
   [Velopack](https://velopack.io), and attaches them to the release: a Windows installer
   (`choom-collector-win-x64-Setup.exe`), a portable zip, a Linux AppImage, and the packages the
   app updates itself from, deltas included.

Windows and Linux use the `win-x64` and `linux-x64` channels, so their files never clash on
the shared release.

To try the packaging without releasing anything: Actions → Release → Run workflow. The
packages are kept as that run's artifacts.

CI builds and tests both platforms on every push.

## License

MIT; see [LICENSE](LICENSE).
