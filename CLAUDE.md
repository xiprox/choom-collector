# Choom Collector

Choom Collector is a desktop app for testing Cyberpunk 2077 mods with friends: the host's mod builds reach every tester's game folder as they're saved, and every tester's logs reach the host's PC, where the host has Claude read them.
Don't write product code until a stage in [the plan](record/areas/product/plan.md) covers it.

## Layout

- Put the product's Rust crates under `src/`, in the one Cargo workspace at the root, as [the spine](record/areas/architecture/01-spine.md#crates) lays them out.
  A new part goes in a crate under `src/`, never a new folder at the root.
- Put the relay in `relay/`: a Cloudflare Worker in TypeScript, forked from fsc-editor's ([decision 003](record/decisions/003-relay-forked-from-fsc-editor.md)).
- `record/` holds the documents agents write.
  [record/index.md](record/index.md) is the map.
  Keep its top level to `index.md`, `decisions/`, and `areas/`, with a folder under `areas/` for each part of the app.
- `experiments/` holds probes, prototypes, and their raw results, in `experiments/areas/<area>/`, the same path the area has under `record/`.
  A finding worth keeping goes in the area's log in `record/areas/<area>/`.
- `.claude/rules/writing.md` is how everything agents write is written, the words in the app included, and it loads into every session.
- `tools/prose-lint.mjs` makes the mechanical checks from writing.md, on Markdown and on the comments in code.
  A hook runs it after every edit, and its findings come back to you as context.
- `.claude/agents/doc-reviewer.md` makes the judgment checks the lint can't.
- `.claude/skills/resume/` picks up an area from its plan and log (`/resume <area>`).

## Prior work

Choom Collector replaces CP77 Log Collector, a C# app that does the same jobs, so that app is the reference for every behavior: its `README.md` describes them and its `src/` holds the code.
It and the other projects below are sibling checkouts on the development machine.
Read and run them, but don't edit their files, commit, or switch branches there.

| Path | What's there | Cite as |
| --- | --- | --- |
| `../cp77-log-collector/` | The C# app: logs to the host, mod sharing, the relay client, game detection on Windows and Linux. | CP77 Log Collector |
| `../../fsc-editor/relay/` and `../../fsc-editor/src/shared/remote-connect.ts` | The relay we forked, a Cloudflare Worker with one Durable Object per code, and the constants and frame types it imports from the second file. | fsc-editor |
| `../../crosscheck/` | Dual Input, where this record's conventions come from. | Dual Input |

When the record uses one of their findings, restate it and cite it as [Evidence](.claude/rules/writing.md#evidence) describes.

## Working rules

**Weigh every mechanism against the simplicity it costs.**
Each rule, mechanism, or special case is one more thing everyone after us has to learn, and can miss.
Add one only for a failure likely enough or bad enough to matter, and cover it with the simplest mechanism that does.
Before leaving a failure uncovered, or taking out a mechanism CP77 Log Collector has, ask the user, and name the gap they accept in the design's known limits.

**Before a test run, give the app its own data folder** with `--data-dir`, and its own session code.
The user's friends play with CP77 Log Collector from `../cp77-log-collector/dist/stable`, so never run, stop, or replace that copy, and never touch the settings in `%APPDATA%\CP77LogCollector\` or its TCP port 47077.

**A commit that meets a plan stage's exit criterion adds the stage's "Done on" line**, as [the plan](record/areas/product/plan.md) says, so the plan never lags the code.

**Commit only the changes you made.**
Other sessions may edit this working tree too, so `git add -A` would put their unfinished work in your commit.
Run `git status --short` before you commit, stage by explicit path, and tell the user which changes you left out.

**Never delete as a side effect.**
Deleting a file, or running `git checkout`, `restore`, `stash`, `reset`, or `clean` on a path, is its own proposal: ask first, and say why.

**Nothing personal in the tree**, because the repository is meant to go public.
No file may carry a username, a home folder path, a machine name, or an email address.
Before committing, run this in Git Bash; no output means it found none:

```bash
git grep -inE --untracked "$(whoami)|$(hostname)|$(git config user.email)"
```

It can't see inside images, so check screenshots by eye before adding them.

## Building

`cargo build` builds the workspace.
The window's crate, `src/app`, compiles gpui, which takes minutes from clean, so iterate with `cargo run -p choom-collector` in the dev profile.
A Linux build needs the X11, Wayland, xkbcommon, and fontconfig development packages, which CI's Ubuntu 22.04 runner installs.
Before a Linux build in WSL, ask the user to run ci.yml's `apt-get install` line there, since it needs their sudo password.

`tools/capture-window.ps1` starts the program, saves a picture of its window, and closes it without taking the focus; `-Click` and `-Hover` act on the window first, for a check of a popover or tooltip.
