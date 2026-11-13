# Network log

## 2026-11-13: The relay runs on the user's Cloudflare account, and passes the same tests there as under wrangler dev

We asked whether the relay, deployed to Cloudflare as `relay/README.md` describes, behaves as it does under `wrangler dev`, now that sessions with friends need one they can reach.
We expected it to, since `wrangler dev` runs the same Worker and Durable Object code, and the free plan's SQLite Durable Objects and rate limit to deploy without changes.

`wrangler deploy` from `relay/` uploaded 6.45 KiB with the `SESSIONS` Durable Object and the `JOIN_LIMIT` rate limit of 20 a minute, to the user's account on its `workers.dev` subdomain (measured on 2026-11-13):

- A plain HTTPS request to it got 426 Upgrade Required, as the Worker answers anything that isn't a WebSocket.
- `COOP_TEST_RELAY` pointed at it, `cargo test -p coop-core --test relay -- --ignored` passed all 3 tests in 4.49s: a host and two testers exchanging messages, a code nobody hosts, and a host that returns within 30s keeping its testers.
- A host and a tester ran from the debug build with the address entered as a player would, with no scheme; the title bar showed 18ms to the relay, logs reached "Up to date", and a line the host saved into a followed mod was in the tester's copy 1.53s later.

The address stays out of this repository, which is public, since it's the user's own relay on their free plan; players get it in the invite.

The user then moved the relay to a subdomain of their own domain, with `wrangler deploy --domain`, so the domain stays out of `relay/wrangler.toml` too.
Since the file doesn't set `workers_dev`, that deploy turned the `workers.dev` address off, and it answered 404 afterwards.
Through Cloudflare's address for the new name, a plain request got 426, and a WebSocket request to `/join` got 101 Switching Protocols.
This PC's router kept answering "no such name", from a lookup just before the deploy, for over an hour, past the zone's 1800s negative-cache time, while 1.1.1.1 and 8.8.8.8 already returned the name (unverified why: possibly a filter on new names in the router or the ISP's resolver).
After the user pointed this PC at 1.1.1.1, `cargo test -p coop-core --test relay -- --ignored` passed 3 of 3 in 4.48s against the new name, and the user's installed 0.3.0 hosted through it, 144ms to the relay.
So a player whose resolver holds a stale "no such name" needs another resolver, not a fix in the app.

## 2026-11-13: A host and two testers on one PC find each other, and a restarted tester rejoins in its own place

We asked whether the session, as [relay and sessions](01-relay-and-sessions.md) describes it, works end to end through the relay: hosting and joining on start, `Hello` and `Welcome`, the players list, and a tester's restart.
We expected the players list to need a second look after a restart, since the relay gives the returning tester a new ID.

Three copies of the app ran on one PC against the relay under `wrangler dev`, each with its own data folder whose settings already held a role, the relay, and one code, and each with its own `--name` (measured on Windows 11, 2026-11-13):

- The host hosted and both testers joined with no setup and no clicks; the host's window listed xip, bossun, and mira, and each tester's window listed the same three, with "Hosting" beside xip and "You" beside themselves.
- Killing bossun's process showed bossun as "Reconnecting" on the host 2s later.
- Starting bossun again with the same data folder put bossun back in its place in the list, no longer reconnecting, within 6s: the host knew it by its install ID.
- A long logs folder path pushed the relay and the version out of the status bar, and a player with nothing received read "1 KB"; both are fixed.

The pictures show this PC's scratchpad path in the status bar, so they aren't in the repository.

## 2026-11-13: The forked relay carries binary frames between a host and two testers, and keeps the testers through a host's drop

We asked whether `relay/`, forked from fsc-editor's relay with binary data frames, does what [relay and sessions](01-relay-and-sessions.md) says, using `coop-core`'s relay client.
We expected the routing byte and the 16-byte tester ID to need a fix or two, since the fork changed the frame format.

`src/core/tests/relay.rs` ran its three tests against the relay under `wrangler dev`, Wrangler 4.123.0, and all three passed in 2.16s (measured on Windows 11, 2026-11-13):

- A host and two testers: a tester's message reached the host with that tester's ID in front, a message to one tester reached only that tester, a message to everyone reached both, the host heard a tester leave, and a host's stop ended the session for the other with `host-stopped`.
- A host that dropped without closing its socket, and opened the same code with its resume token 2s later, kept its tester, and messages flowed both ways; opening the code without the token while the host was away was refused with 403.
- Joining a code nobody hosts closed the socket with `unknown-code`, which the client reports.

The relay's join limit doesn't exist under `wrangler dev`, so it's untested.
This settles the TBD on refused codes in [relay and sessions](01-relay-and-sessions.md#codes).
