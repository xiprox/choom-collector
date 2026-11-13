# Aims

```text
Purpose:     Who Cyberpunk Co-op Tools is for, what it does for them, and what it won't do.
Depends on:  nothing
Decides:     The users, the two jobs, what's out of scope, and the relay and license commitments.
```

Cyberpunk Co-op Tools serves a mod author testing their mods with friends in a co-op session of Cyberpunk 2077.
It does two jobs, both without anyone's hands on it once a session runs: the author's mod builds reach every tester's game folder as they're saved, and every tester's logs reach the author's PC, where the author has Claude read them.

## Who it's for

- **The host**, usually the mod author.
  They build mods, host the session, and want every tester on their latest build and every tester's logs in one folder, without asking anyone to copy files.
- **The testers**, the author's friends.
  They play, and want to set the app up once, follow the mods they're asked to, and otherwise forget it's running.

Anyone in a session can share a mod, but the host sharing their own builds is the common case.

## What it does

- **Builds go out.**
  A player shares a mod with one click, from a list of the mods the app finds in their game folder.
  The others follow it with one click, and from then on their copy matches the sharer's within seconds of each save.
- **Logs come back.**
  The app finds every log the game and its mods write, sends recent history when a tester joins, and then sends each new line as it's written.
  The host gets one folder per session, with a folder per player inside, and points Claude at it.
- **It shows what's flowing.**
  The window answers the host's question at a glance: is everyone on my build, and are their logs reaching me.

## What it won't do

- **It isn't a log viewer.**
  The host reads logs through Claude, so the window shows which logs are picked up and where they land, not their contents.
- **It isn't a mod manager.**
  It doesn't install mods from Nexus, order them, or resolve conflicts; it copies the folders a sharer shares.
- **It isn't for other games.**
  Everything about the game is code in one place, and another game's community can fork it ([decision 001](../../decisions/001-rust-app-of-its-own.md)).

## Commitments

- Every connection goes through a relay the host deploys on their own Cloudflare account, so nobody pays for anyone else's sessions ([decision 003](../../decisions/003-relay-forked-from-fsc-editor.md)).
- The source is open, under a license the user hasn't picked yet (TBD).
