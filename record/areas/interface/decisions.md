# Interface: tentative decisions

What the window's design has settled, from six rounds of mockups with the user on 2026-11-13.
The frames the user settled on are saved in `experiments/areas/interface/frames/`, as HTML, and the values here are the ones those frames draw with.
[The interface log](log.md#2026-11-13-six-rounds-of-mockups-settled-the-window-two-panels-one-for-every-role-plain-with-color-that-means-something) holds the rounds that led to each decision, and what the user said in them.
Stage 2 of [the plan](../product/plan.md) built these, and stage 9 matches the real window to the frames pixel by pixel.

## One window serves every role

- **Decided:** the host and the testers see the same window, and the role changes three things.
  The title bar says "Hosting" with Copy invite and Stop for the host, and "Hosted by xip" with Leave for a tester.
  The status bar shows where everyone's logs go, with copy and open buttons, for the host, and "Your logs go to xip" with its progress for a tester.
  The host can open any player's details, a tester only their own, since only the host receives logs.
- **Because:** anyone can share a mod, so a tester who shares needs the same delivery state the host has, and a tester gains from seeing who's in the session (round 6 of the log entry).

## The window is two panels: players on the left, mods on the right

- **Decided:**
  - The left panel, 280px wide, lists the players, each with one line of state, and below them the selected player's details: game version and store, system, when they joined, data received or sent, and the logs picked up on their PC with a count for each source.
  - A player's game version shows on their row only when it differs from the host's.
  - The right panel lists "Shared in this session" with the columns Follow, Mod, Shared by, Status, and Changed, then "Your mods" with its count, folded until opened.
- **Because:** the host's question, "is everyone on my build, and are their logs reaching me", needs players and mods on one screen, and tabs split it across two (round 5 of the log entry).
  The window is glanced at between test runs while the game runs fullscreen, so a glance answers everything without a click.
- **Leaves out:** a log viewer, since the host reads logs through Claude ([aims](../product/01-aims.md#what-it-wont-do)), and remembering whether "Your mods" was left open, which waits for settings in stage 4.

## A sharer sees each follower's state on their own mods

- **Decided:** a mod the player shares shows a tile per follower in its Status cell, 20 by 16px with the follower's initial, colored by state, and the count up to date beside it, "0 of 3 up to date"; a hover lists each follower's state in words.

  | State | Tile |
  | --- | --- |
  | Up to date | Done |
  | Fetching n of m files | Accent |
  | Has it, needs a game restart | Warning |
  | Paused while reconnecting | Control, with a muted letter |

- **Because:** the user wanted delivery readable without a hover, and color that carries information keeps the screen from blending into gray (round 5 of the log entry).

## Setup happens once, and the app starts its session by itself

- **Decided:**
  - The first run shows a setup screen with the mark, the name, and the line "The host's mod builds go out to everyone, and everyone's logs go back to the host", then Join or Host.
  - Join asks for the invite, which takes a code or `relay.example.net/K7R-M2Q` and fills the relay field from the latter, and shows the game folder found, with Change.
  - Host asks for the relay, with a link to the deploy guide, shows the code with New code, and the logs folder with Change.
  - The name comes from Steam and shows as a sentence, "You'll show up as tetsu, your Steam name", with no field.
  - After setup the app hosts or joins by itself whenever it starts.
- **Because:** testers set the app up once and forget it, and the invite stays just a code once the relay is set (round 6 of the log entry).

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

  - The accent marks what's the player's own or happening now: the mark, the selection on the focused row, checked checkboxes, fetching, and the one filled button on a screen.
  - Nothing is green: what's fine stays gray.
  - Warning and trouble color text and icons, and never fill a box.
  - Buttons are plain by default, and show what they do in a tooltip.
- **Because:** the user found the first rounds felt like a web app, then like a game launcher, and wanted Zed's "no-nonsense feel" without becoming "a plain zed copy and too plain / colorless / everything blending together" (rounds 1, 2, and 5 of the log entry).
  Zed's color comes from syntax highlighting, which a list of mods and players doesn't have, so color here carries meaning instead.
- **Leaves out:** a light theme, since the user didn't ask for one, and IBM Plex itself until its font files are in the repository: the window draws with the system's fonts meanwhile ([log](log.md#2026-11-13-the-window-draws-every-frame-in-gpui-from-made-up-state)).

## Following and unfollowing ask first

- **Decided:**
  - Ticking Follow on a mod opens a box under its row, "Follow EquipmentEx from bossun?", with "It has a DLL, which runs inside your game. Follow only people you trust." for a mod with a DLL, and "Mods run inside your game. Follow only people you trust." for one without, then Follow and Cancel.
  - Unticking it asks "Stop following EquipmentEx?", "Your copy can stay as it is, or go, after a copy goes to the backups.", with Keep my copy and Delete it, the second in trouble's color.
  - A loose file named after a mod marks the mod's row with a warning, and its tooltip says where to move the file.
- **Because:** mods run code inside the game, and CP77 Log Collector asks the same questions (read in CP77 Log Collector, README.md, "Sharing mods").

## Not drawn yet

- The Settings window: the host's extra paths and the sharer's own patterns to leave out, the logs folder, the relay and code, and the version.
- How an available update shows.
- The window before anyone has shared a mod, and a session with one player.
