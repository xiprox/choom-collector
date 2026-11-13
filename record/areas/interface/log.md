# Interface log

## 2026-11-13: The window draws every frame in gpui from made-up state

We asked whether gpui and gpui-component can draw the frames in `experiments/areas/interface/frames/` as they are, fed by `coop-core`'s snapshots, before any session is real.
We expected custom pieces, the delivery tiles and the box that asks before following, to need gpui-component's popover.

`--demo host`, `--demo tester`, `--demo setup-join`, and `--demo setup-host` draw the four frames, and the pictures `tools/capture-window.ps1` took of them are in `experiments/areas/interface/stage-2/` (measured on Windows 11 at 96 dpi, 2026-11-13).

- Every element the frames show is there: the title bar with the session and its buttons, gpui-component's own window buttons, the players and their states, the selected player's details with the logs picked up, the shared mods with checkboxes and tiles, "Your mods" folded, the status bar, and both setup screens.
- A click on EquipmentEx's checkbox, posted with `-Click`, opens the question under its row with the DLL warning, and a resting pointer, posted with `-Hover`, opens the tiles' tooltip with each follower's state.
- The question is a plain absolutely placed box, since the rows are all 26px tall; no popover was needed.
- gpui-component's `TitleBar` keeps the system's window buttons and their hit areas on Windows, so the window moves, minimizes, and closes as any other.
- Text draws in the system's fonts, Segoe UI and Consolas, because IBM Plex's files aren't in the repository yet.
- In Rust 2024, a helper that returns `impl IntoElement` borrows every argument it was given, `cx` included, so the window's helpers return `AnyElement`.

The window's layout matches the frames in structure; the pixel pass is stage 9's.

## 2026-11-13: gpui-kit builds every control the window needs on Windows, and the exe is 16.65 MB

We asked whether gpui, with Longbridge's gpui-component, can draw the window the app needs on Windows and Linux, how big the program gets, and how long it takes to build.
We expected gpui alone to need hand-made controls.

We built a throwaway window in `experiments/areas/interface/gpui-spike/`: a code input and Start button, a scrollable table of 60 made-up mods whose checkboxes toggle, a tooltip with a folder list on a row, and a status line a worker thread updates every second through an `async-channel` and gpui's `cx.spawn`.
It also calls `velopack::VelopackApp::build().run()` first, to prove the two link together.

Found, on Windows 11 with a 16-thread CPU (measured 2026-11-13):

| Measure | Value |
| --- | --- |
| Clean release build, `opt-level = "s"`, fat LTO, 1 codegen unit, stripped | 4m10s, 123s of it the final link |
| Release rebuild after a one-line change | 2m03s |
| Dev build, clean and after a one-line change | 1m57s and 13s |
| Program size at `opt-level` `"s"`, `"z"`, and `3` | 16.65 MB, 14.75 MB, and 22.82 MB |
| Time to the window | about 0.4s |
| CPU while idle, with the line updating every second | 0.15% to 0.6% of one core |
| Memory | about 70 MB working set, 92 MB private |

- Every control came from crates.io: `gpui-kit` 0.9.1, which pins `gpui-pre` 0.3.12, a weekly snapshot of Zed's gpui published by a Longbridge maintainer, not by Zed; the official `gpui` crate is 0.2.2.
- Everything it pulled in is under a permissive license; Zed's own `ui`, `theme`, and `settings` crates are GPL-3.0-or-later, so we can't copy from them.
- Clicking a checkbox in a table row also selects the row, and a checkbox in a table cell needs listener and delegate code to toggle.
- `gpui-kit`'s README describes 0.7 while the crate is 0.9.1, so the API was learned from its source.
- The Linux build stopped before compiling: `yeslogic-fontconfig-sys` needs `pkg-config`, and installing it in WSL needs a password the agent didn't have.
  The packages it needs are `pkg-config libfontconfig-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-dev libx11-xcb-dev libxcb1-dev libwayland-dev`.
  Its size and libraries are unmeasured, and so is the Steam Deck.

This led to [decision 002](../../decisions/002-gpui-through-gpui-kit.md), and to CI installing those packages on its Linux runner.

## 2026-11-13: Six rounds of mockups settled the window: two panels, one for every role, plain with color that means something

We asked the user what the rewritten app should look like, drawing mockups on a design canvas and changing them on the user's calls.
We expected the first round's four directions to settle the look, and a round of polish to finish it.

1. **Four directions:** an editor like Zed, a themed Night City HUD, a light calm one, and a keyboard-first console.
   The user picked the editor, and said it still felt like a web app.
2. **The editor as a desktop window,** with a title bar, panels, tabs, and a status bar.
   The user said it looked vibe coded; the causes we found were values picked element by element, the default dark-with-blue palette, color on every row, and stock badges and dots.
3. **A system first,** with its own type, color, mark, and words; Dual Input's settled look was ruled out for this product (user, 2026-11-13).
4. **The use case corrected:** the user explained that logs only need to reach the host, who has Claude read them, and that mods are the author's builds going out, so the log viewer and its problems panel went.
   The user kept CP77 Log Collector's list of found mods with one-click sharing over our proposal to pick folders, since it needs no file dialog and merges a mod's folders across places.
5. **Two panels, plain or middle:** the user found the tabbed attempt felt like a game launcher, and wanted Zed's no-nonsense feel without everything blending together.
   We drew the same two-panel window twice: plain, in neutral grays with color only on problems and a muted blue selection, and middle, which adds a toned-down yellow accent for what's the player's own or happening now, and a tile per follower colored by their state.
   The user picked middle.
6. **One window for every role,** "Your mods" folded, setup done once, no per-player send options, extra paths set by the host, and the name from Steam with no field.

What we settled is in [the interface decisions](decisions.md), and the last round's frames are in `experiments/areas/interface/frames/`.
We decided not to make the HTML frames pixel-perfect, since gpui lays out and draws text its own way; stage 9 does the pixel pass on the real window (user, 2026-11-13).
