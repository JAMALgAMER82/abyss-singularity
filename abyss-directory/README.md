# Abyss Singularity — directory service

A tiny Cloudflare Worker that gives Abyss its GameRanger-style global
presence: who's online right now, friend requests, DMs, global chat.

This is the **only piece of infrastructure** Abyss needs to run. The rest
(games, emulators, streaming, chat between friends) stays peer-to-peer
over the embedded Tailscale mesh. The directory only handles discovery —
once two users become friends here, they exchange Tailscale auth keys
and from then on talk to each other directly.

## What it provides

| Endpoint | Purpose |
|---|---|
| `POST /v1/presence` | Heartbeat — keep me on the online list |
| `GET  /v1/online` | List users seen recently (default 5 min window) |
| `POST /v1/hidden` | Toggle "appear offline" |
| `POST /v1/friend-request` | Send an invite (carries your tailnet auth key) |
| `GET  /v1/friend-requests` | Inbox — pending requests addressed to me |
| `GET  /v1/friend-responses` | Did my outgoing requests get accepted? |
| `POST /v1/friend-accept` | Accept a request (carries my tailnet auth key back) |
| `POST /v1/friend-reject` | Decline a request |
| `GET  /v1/friends` | My full friend list |
| `POST /v1/dm` / `GET /v1/dm` | Direct messages (store-and-forward) |
| `POST /v1/global-chat` / `GET /v1/global-chat` | IRC-style global lobby |
| `POST /v1/block` / `POST /v1/unblock` | Block list |

## Free-tier reality

| Resource | Free tier | Our usage at small scale |
|---|---|---|
| Workers requests | 100k/day | <2k/day for 50 users on 5-min heartbeat |
| D1 writes | 100k/day | <500/user/day (presence + occasional friend ops) |
| D1 reads | 5M/day | Plenty |
| D1 storage | 5GB | <1MB for a friend group |

→ **No credit card needed** for friend-group-scale deployments. Cloudflare
Pro ($5/mo) opens 10× ceilings if you ever need them.

## One-time setup

You need a Cloudflare account (free). Then:

```sh
# 1. Install deps
npm install

# 2. Log in to Cloudflare
npx wrangler login

# 3. Create the D1 database
npx wrangler d1 create abyss-directory
#    → prints a UUID — paste it into wrangler.toml's database_id field.

# 4. Initialise the schema
npm run init-db
#    (alternatively use `init-db-local` for a local dev copy)

# 5. Deploy
npm run deploy
#    → prints your Worker URL, e.g. https://abyss-directory.you.workers.dev
```

Tell Abyss your Worker URL by setting the `ABYSS_DIRECTORY_URL` env var
before building, OR — once the in-app Settings → Directory panel ships —
paste it into the field there at runtime.

## Local dev

```sh
npm run dev
# wrangler starts on http://localhost:8787 with a local D1 instance.
# Run `npm run init-db-local` once first to seed the schema.
```

## Privacy & abuse model

- No accounts, no email — identity is a client-generated UUID + a chosen
  display handle. The UUID is the bearer of identity; if a friend forgets
  it they're someone new from the directory's perspective.
- Friend requests are rate-limited to 20/hour/sender. DMs to 60/min/sender.
  Global chat to 6/min/sender. Anyone hammering the API trips a 429.
- `appear offline` (POST /v1/hidden) removes you from `/v1/online` lists.
  You can still send/receive friend requests and DMs.
- `block` hides a user's chat / DMs from you and removes them from your
  online list. Blocks are unilateral.
- The Worker scheduled job prunes DMs >7d old and global chat >30d old
  every 24 h.

## What's NOT here

- No voice / video chat. Out of scope for a free-tier Worker; would
  need separate WebRTC TURN infrastructure.
- No file transfer. Already works peer-to-peer through Abyss's existing
  transfer system once friends are mesh-paired.
- No game-server hosting / matchmaking beyond "user X is online". Real
  netplay still uses Abyss's existing lobby flow over the tailnet.

## License

Same as the rest of Abyss Singularity. See repo root.
