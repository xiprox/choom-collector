# The relay

Cyberpunk Co-op Tools connects players through a relay the host runs: a Cloudflare Worker that passes each player's messages on and keeps none of them.
It runs on Cloudflare's free plan, and one relay serves as many sessions as its host wants.
A tester needs nothing from this page: the host's invite carries the relay's address.

## Deploy your own

You need a Cloudflare account, which is free, and [Node.js](https://nodejs.org/) 22 or newer.

1. Download this repository, as a zip from GitHub or with `git clone`, and open a terminal in its `relay` folder.
2. Install the relay's tools:

   ```bash
   npm install
   ```

   You'll see a `node_modules` folder appear.
3. Sign in to Cloudflare:

   ```bash
   npx wrangler login
   ```

   A browser page asks you to allow Wrangler, Cloudflare's command-line tool; once you allow it, the terminal says you're logged in.
4. Deploy the relay:

   ```bash
   npx wrangler deploy
   ```

   The last lines name its address, such as `https://cyberpunk-coop-tools-relay.<your-subdomain>.workers.dev`.
5. In the app's setup, choose Host, and enter that address without `https://`: `cyberpunk-coop-tools-relay.<your-subdomain>.workers.dev`.

To update the relay after pulling a newer version of this repository, run `npx wrangler deploy` again in this folder.

## What it does

[Relay and sessions](../record/areas/network/01-relay-and-sessions.md) describes its requests, frames, and limits.
It's forked from fsc-editor's Remote Connect relay, and changed to carry the app's messages as binary frames.

## Running it on your PC

```bash
npm run dev
```

Wrangler serves the relay at `ws://127.0.0.1:8787`, which the app and `coop-core`'s relay test take as a relay address.
