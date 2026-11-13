# Relay and sessions

```text
Purpose:     How players reach each other through the relay, and how a session starts, runs, and recovers.
Depends on:  decisions/003-relay-forked-from-fsc-editor.md, product/02-v1-scope.md
Decides:     The relay's endpoints and frames, codes and resume tokens, the first messages, how players learn about each other, and reconnecting.
```

Every player's app opens one WebSocket to the host's relay: the host's to `/host`, each tester's to `/join`.
The relay holds a session per code, passes the testers' messages to the host and the host's messages to the testers, and reads only the routing bytes at the front of a frame.
Testers never reach each other, so everything one tester sends another goes through the host's app.

## The relay

The relay is fsc-editor's Worker, forked into `relay/` with the constants it imports (read in fsc-editor, `relay/src/index.ts` and `src/shared/remote-connect.ts`).
It keeps one Durable Object per code, rebuilds its list of sockets from the sockets themselves after it hibernates, and costs nothing while idle.

| Request | What happens |
| --- | --- |
| `GET /host?code=<code>` | Opens a session under the code, and answers on the socket with `{"type":"hosting","code":…,"resume":…}`, the resume token. A code whose session already has its host connected gets 409. |
| `GET /host?code=<code>&resume=<token>` | Takes a live session back after a drop. A request without the token the relay handed out is refused with 403. |
| `GET /join?code=<code>` | Adds a tester, answering `{"type":"joined","guestId":…}`, and tells the host `{"type":"guest-joined","guestId":…}`. More than 8 testers, or a code with no host, closes the socket with the reason. |

- Control frames are JSON text: `hosting`, `joined`, `guest-joined`, `guest-left`, and `closed` with its reason.
- **Changed from fsc-editor:** data frames are binary, where fsc-editor's frames are JSON, with the message as a JSON value.
  A tester's frame is the message itself, and the relay puts the tester's `guestId`, a UUID, in front of it as its 16 bytes before passing it to the host.
  A host's frame starts with one byte: 0 sends the message after it to every tester, and 1 sends it to the tester whose 16-byte `guestId` follows.
- When the host's socket drops, the relay waits 30s for the same host to come back before ending the session, and a host that stops on purpose closes with code 4001, which ends the session at once.
- The relay limits joins to 20 a minute from one IP address, counted per Cloudflare location (read in fsc-editor, `relay/wrangler.toml`).

## Codes

- A code is 6 characters from `23456789ABCDEFGHJKMNPQRSTVWXYZ`, shown as `K7R-M2Q`.
  A player types it with or without the hyphen, in either case.
- **Changed from fsc-editor:** the host's PC picks its code at random and keeps it, where fsc-editor's relay picks a code for every `/host` without one, so the same code works session after session and testers type it once.
- The resume token proves the host only while the session lives.
  When the session ends the relay deletes the token, and the code is free for the next host that opens it (read in fsc-editor, `relay/src/index.ts`, `endSession`).
  With about 729 million codes, another host picking the same one is unlikely, but possible.
- When another host holds a host's kept code, the relay refuses it with 409 or 403, and the host's window says "another host is using this code" and offers New code.

## The first messages

The messages players exchange are Rust types in `coop-proto`, encoded with `postcard`.

1. A tester's first message is `Hello`: the protocol version, the app's version, the player's install ID, name, game version, and system.
2. The host answers `Welcome`: the host's name, the extra paths every tester picks up, and what the host already has of this tester's logs, so the tester resumes instead of sending again.
   A tester on another protocol version gets `Refused`, with the version to update to.
3. Whenever a player joins, leaves, or changes state, the host sends every tester `Players`: each player's ID, name, game version, system, and state.

Logs and mods messages come with their stages' design docs.

## Reconnecting

- A tester whose socket drops tries again after 500ms, 1.5s, 4s, and 8s, then every 8s, and resumes from the `Welcome` it gets.
  CP77 Log Collector's testers wait 5s, or 1s when the host asks, and its host retries at 500ms, 1.5s, 4s, and 8s (read in CP77 Log Collector, src/Sender.cs and src/Relay.cs).
- A host whose socket drops reconnects with its code and resume token.
  Within 30s it keeps its testers, and tells them to resend from what it has, because the relay drops what testers send while the host is away (read in CP77 Log Collector, README.md, "Getting the others connected").
- A host that restarts hosts its code again, and testers waiting for it join by themselves.
- TBD: how often players ping each other, and how long silence counts as a drop; stage 4 measures what the relay's own WebSocket keeps alive.
