# Network log

## 2026-11-13: The forked relay carries binary frames between a host and two testers, and keeps the testers through a host's drop

We asked whether `relay/`, forked from fsc-editor's relay with binary data frames, does what [relay and sessions](01-relay-and-sessions.md) says, using `coop-core`'s relay client.
We expected the routing byte and the 16-byte tester ID to need a fix or two, since the fork changed the frame format.

`src/core/tests/relay.rs` ran its three tests against the relay under `wrangler dev`, Wrangler 4.123.0, and all three passed in 2.16s (measured on Windows 11, 2026-11-13):

- A host and two testers: a tester's message reached the host with that tester's ID in front, a message to one tester reached only that tester, a message to everyone reached both, the host heard a tester leave, and a host's stop ended the session for the other with `host-stopped`.
- A host that dropped without closing its socket, and opened the same code with its resume token 2s later, kept its tester, and messages flowed both ways; opening the code without the token while the host was away was refused with 403.
- Joining a code nobody hosts closed the socket with `unknown-code`, which the client reports.

The relay's join limit doesn't exist under `wrangler dev`, so it's untested.
This settles the TBD on refused codes in [relay and sessions](01-relay-and-sessions.md#codes).
