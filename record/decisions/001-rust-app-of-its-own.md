# 001: Cyberpunk Co-op Tools is a Rust app of its own, for Cyberpunk 2077 only

Cyberpunk Co-op Tools is written from scratch in Rust, in a repository of its own, and does the jobs CP77 Log Collector does for the game and only that game.
It replaces CP77 Log Collector once it does everything [the v1 scope](../areas/product/02-v1-scope.md) lists, and until then the user's friends keep playing with the C# app.
Decided by the user on 2026-11-13.

## Context

CP77 Log Collector started as a log collector and grew a mod-sharing feature, and its window grew with it, until the user found it convoluted.
The user wanted to rethink its design, write it in Rust, open its source, and let anyone self-host the relay it needs.
The C# app is about 6,000 non-blank lines across 11 files, the largest being `src/Mods.cs` at 1,535 and `src/MainWindow.cs` at 1,112 (counted in CP77 Log Collector 1.3.1, 2026-11-13).

## Options

1. **Rework the C# app in place.**
   It keeps every behavior that works, but keeps the .NET runtime in each 21 MB download and the history of its design.
2. **Rewrite it in Rust as a general tool for any game, with games as data files.**
   Other games would cost a profile each, but every feature would pay for a layer it doesn't need while one game uses it.
3. **Rewrite it in Rust for Cyberpunk 2077 only, with features built on shared parts inside.**
   Everything about the game is code in one place, and another game's community can fork it, but every behavior in those 6,000 lines is built and proved again, and a group switches apps all at once.

## Decision

Option 3.
The user wanted it "generic in features, but still explicitly for cyberpunk. any other game want it? fork it" (user, 2026-11-13).

## What it commits us to

- CP77 Log Collector is the reference for every behavior the scope keeps, and a behavior it has that the scope drops is dropped on purpose, as [CLAUDE.md](../../CLAUDE.md#working-rules) requires.
- The protocol starts over: Cyberpunk Co-op Tools doesn't talk to CP77 Log Collector, so a group moves from one to the other together.
- The repository is meant to go public, so nothing personal goes in it.
  Its license is open (TBD: the user hasn't picked one).
