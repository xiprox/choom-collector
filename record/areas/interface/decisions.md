# Interface: tentative decisions

What the window's design has settled, from six rounds of mockups with the user on 2026-11-13.
The frames the user settled on are saved in `experiments/areas/interface/frames/`, as HTML, and the values here are the ones those frames draw with; [the interface log](log.md) holds the rounds that led to them.
Stage 2 of [the plan](../product/plan.md) builds these, and stage 9 matches the real window to the frames pixel by pixel.

## One window serves every role

- **Decided:** the host and the testers see the same window, and the role changes three things.
  The title bar says "Hosting" with Copy invite and Stop for the host, and "Hosted by xip" with Leave for a tester.
  The status bar shows where everyone's logs go, with copy and open buttons, for the host, and "Your logs go to xip" with its progress for a tester.
  The host can open any player's details, a tester only their own, since only the host receives logs.
- **Because:** anyone can share a mod, so a tester who shares needs the same delivery state the host has, and a tester gains from seeing who's in the session.
- Settled by the user on 2026-11-13.

## The window is two panels: players on the left, mods on the right

- **Decided:**
  - The left panel, 280px wide, lists the players, each with one line of state, and below them the selected player's details: game version and store, system, when they joined, data received or sent, and the logs picked up on their PC with a count for each source.
  - A player's game version shows on their row only when it differs from the host's.
  - The right panel lists "Shared in this session" with the columns Follow, Mod, Shared by, Status, and Changed, then "Your mods" with its count, folded until opened, and remembered open or folded.
- **Because:** the host's question, "is everyone on my build, and are their logs reaching me", needs players and mods on one screen, and tabs split it across two (user, 2026-11-13).
  The window is glanced at between test runs while the game runs fullscreen, so a glance answers everything without a click.
- **Leaves out:** a log viewer, since the host reads logs through Claude ([aims](../product/01-aims.md#what-it-wont-do)).
- Settled by the user on 2026-11-13.

## A sharer sees each follower's state on their own mods

- **Decided:** a mod the player shares shows a tile per follower in its Status cell, 20 by 16px with the follower's initial, colored by state, and the count up to date beside it, "0 of 3 up to date"; a hover lists each follower's state in words.
  | State | Tile |
  | --- | --- |
  | Up to date | Done gray |
  | Fetching n of m files | Accent |
  | Has it, needs a game restart | Warning |
  | Paused while reconnecting | Control gray, muted letter |
- **Because:** the user wanted delivery visible without a hover, and color that carries information keeps the screen from blending into gray.
- Settled by the user on 2026-11-13.

## Setup happens once, and the app starts its session by itself

- **Decided:**
  - The first run shows a setup screen with the mark, the name, and the line "The host's mod builds go out to everyone, and everyone's logs go back to the host", then Join or Host.
  - Join asks for the invite, which takes a code or `relay.example.net/K7R-M2Q` and fills the relay field from the latter, and shows the game folder found, with Change.
  - Host asks for the relay, with a link to the deploy guide, shows the code with New code, and the logs folder with Change.
  - The name comes from Steam and shows as a sentence, "You'll show up as tetsu, your Steam name", with no field.
  - After setup the app hosts or joins by itself whenever it starts.
- Settled by the user on 2026-11-13.

## The look is dense and plain, with color only where it means something

- **Decided:** the window draws with these values.

  | Role | Value |
  | --- | --- |
  | Base, the mods panel | `#15171b` |
  | Panel, the players panel and bars | `#1a1d22` |
  | Raised, selection and popovers | `#232830` |
  | Line | `#272b32` |
  | Line between rows | `#1e2126` |
  | Control, borders and empty checkboxes | `#3a404a` |
  | Text | `#dfe2e7` |
  | Strong text | `#f4f5f7` |
  | Muted text | `#959cab` |
  | Accent: what's yours or happening now | `#ddc94f`, with `#15171b` on it |
  | Done | `#8b94a3` |
  | Warning: something's off | `#e89b5e` |
  | Trouble: something broke | `#e5736e` |

  | Thing | Value |
  | --- | --- |
  | Words | IBM Plex Sans, 13px, and 12px for labels and details |
  | Codes, sizes, and paths | IBM Plex Mono, 12px or 13px |
  | The setup screen's name | IBM Plex Sans, 16px, semibold |
  | Title bar, section headers, status bar | 32px, 28px, 24px tall |
  | Rows | 26px tall |
  | Buttons and inputs | 24px or 28px tall |
  | Corners | 3px; the window's from the system |
  | Icons | Lucide, as gpui-kit ships it, 14px |

  - The accent marks what's the player's own or happening now: the mark, the selection on the focused list, checked checkboxes, fetching, and the one filled button on a screen.
  - Nothing is green: what's fine stays gray.
  - Warning and trouble color text and icons, and never fill a box.
  - Buttons are plain by default, and show their shortcut in the tooltip.
- **Because:** the user found the first rounds felt like a web app, then like a game launcher, and wanted Zed's "no-nonsense feel" without becoming "a plain zed copy and too plain / colorless / everything blending together" (user, 2026-11-13).
  A list of mods and players has none of the color code brings to Zed, so the color here carries meaning instead.
- **Leaves out:** a light theme, since the user didn't ask for one.
- Settled by the user on 2026-11-13.

## Following and unfollowing ask first

- **Decided:**
  - Ticking Follow on a mod opens a box under it, "Follow EquipmentEx from bossun?", and for a mod with a DLL, "It has a DLL, which runs inside your game. Follow only people you trust.", with Follow and Cancel.
  - A loose file named after a mod shows a warning in the mod's row, and its tooltip says where to move it.
  - Unfollowing asks whether to keep or delete the copy (TBD: drawn in stage 2).
- Settled by the user on 2026-11-13.

## Not drawn yet

- The Settings window: the host's extra paths and the sharer's own patterns to leave out, the logs folder, the relay and code, and the version.
- How an available update shows.
- The unfollow question, and the window before anyone has shared a mod.
