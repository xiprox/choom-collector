# 003: Every connection goes through a relay the host runs, forked from fsc-editor's

Players reach each other only through a relay: fsc-editor's Cloudflare Worker, forked into `relay/` and changed freely for this app.
The host deploys one on their own Cloudflare account, and each player enters its address once.
Decided by the user on 2026-11-13.

## Context

CP77 Log Collector connects through fsc-editor's relay at the user's own address, or directly to the host's TCP port 47077, which needs a port forward, UPnP, or a VPN (read in CP77 Log Collector, README.md, "Getting the others connected").
An app meant for anyone can't send everyone's traffic through the user's relay, at the user's cost.
The fsc-editor relay is about 300 lines: one Durable Object per invite code, holding a host and up to 8 guests, passing each message on without reading it (read in fsc-editor, `relay/src/index.ts`).

## Options

1. **Keep using fsc-editor's deployed relay.**
   Nothing to build, but the user pays for everyone, and the relay must stay compatible with fsc-editor's clients.
2. **Write a relay in Rust, as one binary and a container image.**
   It runs on any server, and duplicates a relay that already works.
3. **Fork fsc-editor's Worker, and have each host deploy their own on Cloudflare's free plan.**
4. **Keep the direct TCP connection beside a relay.**

## Decision

Option 3, without option 4.
The user chose Cloudflare because it is free and easy to deploy to, and the app "would require the user to enter a relay url" (user, 2026-11-13).
CP77 Log Collector's relay traffic has stayed within the free plan in the user's sessions, so the fork keeps the relay's design and changes what this app needs.

## What it commits us to

- Direct TCP, port 47077, and UPnP are gone.
  The host's own logs and mods never cross a socket, since the host's app holds them already.
- The relay's protocol is ours to change, and keeps no compatibility with fsc-editor's clients or with CP77 Log Collector.
- `relay/` carries a deploy guide a host can follow without our help.
- Messages pass through the relay readable by whoever runs it, which is the host, so the app doesn't encrypt them.
