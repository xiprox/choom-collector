# Relay and sessions

```text
Purpose:     How players reach each other through the relay, and how a session starts, runs, and recovers.
Depends on:  decisions/003-relay-forked-from-fsc-editor.md, product/02-v1-scope.md
Decides:     The relay's endpoints and frames, codes and resume tokens, the first messages, how players learn about each other, and reconnecting.
```

Every player's app opens one WebSocket to the host's relay: the host's to `/host`, each tester's to `/join`.
The relay holds a session per code, passes the testers' messages to the host and the host's messages to the testers, and reads only the envelope it adds.
Testers never reach each other, so everything one tester sends another goes through the host's app.

## The relay

The relay is fsc-editor's Worker, forked into `relay/` (read in fsc-editor, `relay/src/index.ts`).
It keeps one Durable Object per code, rebuilds its list of sockets from the sockets themselves after it hibernates, and costs nothing while idle.

| Request | What happens |
| --- | --- |
| `GET /host?code=<code>` | Opens a session under the code, and answers on the socket with `{"type":"hosting","code":…,"resume":…}`. |
| `GET /host?code=<code>&resume=<token>` | Takes the session back after a drop. A request without the token the relay handed out is refused with 403, and one for a session whose host is connected with 409. |
| `GET /join?code=<code>` | Adds a tester, answering `{"type":"joined","guestId":…}`, and tells the host `{"type":"guest-joined","guestId":…}`. More than 8 testers, or a code with no host, closes the socket with the reason. |

- Control frames are JSON text: `hosting`, `joined`, `guest-joined`, `guest-left`, and `closed` with its reason.
- **Changed from fsc-editor:** data frames are binary, where fsc-editor wraps base64 in JSON.
  A tester's frame is the message itself, and the relay puts the tester's 16-byte ID in front of it before passing it to the host.
  A host's frame starts with one byte: 0 sends the message after it to every tester, and 1 sends it to the tester whose 16-byte ID follows.
- When the host's socket drops, the relay waits 30s for the same host to come back before closing the session, and a host that stops on purpose closes with code 4001, which ends the session at once.
- The relay limits joins to 20 a minute from one IP address, counted per Cloudflare location (read in fsc-editor, `relay/wrangler.toml`).

## Codes

- A code is 6 characters from `23456789ABCDEFGHJKMNPQRSTVWXYZ`, picked at random on the host's PC, and shown as `K7R-M2Q`.
  A player types it with or without the hyphen, in either case.
- The host keeps its code and the relay's resume token in its settings, so the same code works every session, and only that PC can take it back.

## The first messages

The messages players exchange are Rust types in `coop-proto`, encoded with `postcard`.

1. A tester's first message is `Hello`: the protocol version, the app's version, the player's install ID, name, game version, and system.
2. The host answers `Welcome`: the host's name, the extra paths every tester picks up, and what the host already has of this tester's logs, so the tester resumes instead of sending again.
   A tester on another protocol version gets `Refused`, with the version to update to.
3. Whenever a player joins, leaves, or changes state, the host sends every tester `Players`: each player's ID, name, game version, system, and state.

Logs and mods messages come with their stages' design docs.

## Reconnecting

- A tester whose socket drops tries again after 500ms, 1.5s, 4s, and 8s, then every 8s, and resumes from the `Welcome` it gets.
- A host whose socket drops reconnects with its code and resume token.
  Within 30s it keeps its testers, and tells them to resend from what it has, because the relay drops what testers send while the host is away (read in CP77 Log Collector, README.md, "Getting the others connected").
- A host that restarts hosts the same code, and testers waiting for it join by themselves.
- TBD: how often players ping each other, and how long silence counts as a drop; stage 4 measures what the relay's own WebSocket keeps alive.
