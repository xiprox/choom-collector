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
  | Base, the mods panel | `#08080a` |
  | Panel, the players panel and bars | `#0d0d10` |
  | Raised, selection and popovers | `#18181c` |
  | Line | `#1e1e23` |
  | Line between rows | `#141418` |
  | Control, borders and empty checkboxes | `#303036` |
  | Text | `#e3e3e8` |
  | Strong text | `#f4f4f7` |
  | Muted text | `#96969f` |
  | Placeholders | `#58585f` |
  | Accent: what's yours or happening now | `#ddc94f`, with `#08080a` on it |
  | Done | `#8c8c95` |
  | Warning: something's off | `#e89b5e` |
  | In progress: files on their way | `#74d4ff`, Dual Input's primary, and `#3f7489` for what's still to come |
  | Trouble: something broke | `#ea7977`, as Dual Input's |

  | Thing | Value |
  | --- | --- |
  | Words | IBM Plex Sans, 13px, and 12px for labels and details |
  | Codes, sizes, and paths | IBM Plex Mono, 12px or 13px |
  | The setup screen's name | IBM Plex Sans, 16px, semibold |
  | Title bar, section headers, status bar | 32px, 28px, 24px tall |
  | Rows, section headers, column headings, and the status bar | 32px tall, with content 8px in on every side |
  | Spacing and sizes | multiples of 4px |
  | Buttons and inputs | 24px or 28px tall |
  | Corners | 4px; the window's from the system |
  | Icons | Lucide, as gpui-kit ships it, drawn with a 1.75 stroke at 14px |
  | Buttons | 24px tall, 12px at the sides, medium weight, no border: primary in the accent, secondary in control gray, danger in control gray with trouble's text |

  - The accent marks what's the player's own or happening now: the mark, the selection on the focused row, checked checkboxes, fetching, and the one filled button on a screen.
  - Nothing is green: what's fine stays gray.
  - Warning and trouble color text and icons, and never fill a box.
  - Buttons are plain by default, and show what they do in a tooltip.
- **Because:** the user found the first rounds felt like a web app, then like a game launcher, and wanted Zed's "no-nonsense feel" without becoming "a plain zed copy and too plain / colorless / everything blending together" (rounds 1, 2, and 5 of the log entry).
  Zed's color comes from syntax highlighting, which a list of mods and players doesn't have, so color here carries meaning instead.
  The grays are near-black with a touch of blue: the user asked for a "clean, neutral, professional black" in the polish pass, found exact neutral grays read yellowish, and moved spacing to a 4px grid (2026-11-13).
- **Leaves out:** a light theme, since the user didn't ask for one.
  IBM Plex is built into the program from `assets/fonts/`, under the SIL Open Font License.

## Following and unfollowing ask first

- **Decided:**
  - Ticking Follow on a mod opens a box under its row, "Follow EquipmentEx from bossun?", with "It has a DLL, which runs inside your game. Follow only people you trust." for a mod with a DLL, and "Mods run inside your game. Follow only people you trust." for one without, then Follow and Cancel.
  - Unticking it asks "Stop following EquipmentEx?", "Delete your copy as well?", with Keep my copy, Delete it in trouble's color, and Cancel, which leaves the mod followed (the user, 2026-11-13).
  - A loose file named after a mod marks the mod's row with a warning, and its tooltip says where to move the file.
- **Because:** mods run code inside the game, and CP77 Log Collector asks the same questions (read in CP77 Log Collector, README.md, "Sharing mods").

## Settings is a page the sliders open

- **Decided:**
  - The sliders in the title bar open Settings in place of the session, with Cancel and Save in its title bar.
  - Everyone sees the relay, the code, the patterns never sent from their own shared mods, the version, and the app's log with Show.
    The host also sees New code, the logs folder, and the extra files every player's PC picks up, one a line.
  - Save checks the relay and the code first, and says what's wrong in trouble's color under the title bar.
  - A new relay or code starts the session again with it; the rest applies at once.
- **Because:** the user made the relay a one-time setting, the code one that's remembered, and the extra paths a host setting for every tester (round 6 of [the log entry](log.md#2026-11-13-six-rounds-of-mockups-settled-the-window-two-panels-one-for-every-role-plain-with-color-that-means-something)), so changing any of them is rare and stays off the main window.
  Save and Cancel sit in the title bar because the host's page runs past the window's height ([log](log.md#2026-11-13-settings-and-the-update-button-went-straight-into-the-window-with-no-frames-first)).
- **Leaves out:** the game folder, which Stop or Leave, then setup's Change, already covers.

## An update waits for the player's restart

- **Decided:** the app checks the project's GitHub releases 5s after it starts and every 6 hours after, and downloads what it finds.
  Then "Restart to update" shows in the title bar, filled in the accent, with the version in its tooltip, and beside the version in Settings.
  Nothing installs until the player presses it.
- **Because:** a restart drops the player from the session for a moment, so the player picks when, and the accent marks what's waiting on them.
- **Leaves out:** release notes, which the release's page on GitHub has.

## Not drawn yet

- The window before anyone has shared a mod, and a session with one player.

## The polish pass settled the header, the sidebar, and "Your mods"

- **Decided:**
  - The title bar's left reads the mark, the code, and a red stop square, 6px apart glyph to glyph; clicking the host's code copies it, and it reads "Copied" for a moment.
    A word follows only when the session isn't running as it should: Connecting, Reconnecting, or "Waiting for xip's session".
  - Each player's row starts with their tile, colored by state, and keeps its full sentence on the right; a tile fills from the left while their mods are on their way.
  - A player's details open under their row when it's clicked, on the row's raised background, and close on a second click; none is open by default.
  - "Your mods" is a drawer pinned to the bottom of the mods panel, opening upward to half its height, with up and down chevrons.
  - A question's answers are buttons on the left and Cancel is plain on the right; unfollowing asks "Delete your copy as well?".
- **Because:** the user found the header unbalanced, the sidebar a wall of text, the list jumping when a mod moved into the shared list, and the yellow fetching state clashing with the accent (the polish pass, 2026-11-13).
