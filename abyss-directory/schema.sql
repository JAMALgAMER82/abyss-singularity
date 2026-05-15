-- Abyss Singularity directory — D1 (SQLite) schema.
--
-- Tracks who's online globally, pending friend requests, established
-- friendships, and recent DMs / global chat. Designed to fit comfortably
-- inside Cloudflare's free tier: 5 M reads + 100 k writes / day, 5 GB
-- storage. The hot path is `users.last_seen` upserts every ~5 min per
-- online client.

-- Each Abyss install registers itself with a client-generated UUID and a
-- user-picked display handle. `hidden = 1` means "appear offline" — they
-- can still send/receive friend requests but won't show in /v1/online.
CREATE TABLE IF NOT EXISTS users (
    id           TEXT PRIMARY KEY,             -- UUID generated client-side
    handle       TEXT NOT NULL,                -- display name; not unique
    app_version  TEXT NOT NULL,                -- Abyss CARGO_PKG_VERSION
    country      TEXT,                         -- 2-letter ISO, optional
    hidden       INTEGER NOT NULL DEFAULT 0,   -- 1 = "appear offline"
    created_at   INTEGER NOT NULL,             -- epoch ms
    last_seen    INTEGER NOT NULL              -- epoch ms — updated by heartbeats
);

CREATE INDEX IF NOT EXISTS idx_users_last_seen ON users(last_seen);

-- A friend invite. The requester sends their own Tailscale auth key
-- ("invite_code") inside the request so the recipient can join their
-- tailnet on accept. The recipient sends back THEIR auth key in
-- `accept_invite_code` so the requester can join too — mutual peering.
CREATE TABLE IF NOT EXISTS friend_requests (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id             TEXT NOT NULL,
    to_id               TEXT NOT NULL,
    from_handle         TEXT NOT NULL,
    message             TEXT,
    -- Tailnet auth key the requester is offering (optional). When NULL,
    -- the friendship is directory-only (chat / DM); the pair can still
    -- exchange mesh invites later via a separate DM. Letting users be
    -- friends without committing to mesh-pairing keeps the model simple
    -- when a user has many friends (Tailscale's tsnet only holds one
    -- tailnet identity at a time, so swapping per friendship breaks).
    invite_code         TEXT,
    status              TEXT NOT NULL DEFAULT 'pending',  -- pending | accepted | rejected
    created_at          INTEGER NOT NULL,
    responded_at        INTEGER,
    accept_invite_code  TEXT                    -- recipient's tailnet code (set on accept)
);

CREATE INDEX IF NOT EXISTS idx_fr_to_status ON friend_requests(to_id, status);
CREATE INDEX IF NOT EXISTS idx_fr_from_status ON friend_requests(from_id, status);

-- Symmetric friendship — a_id < b_id by string ordering so each pair is
-- stored exactly once. Lookups by either side use the OR query.
CREATE TABLE IF NOT EXISTS friendships (
    a_id            TEXT NOT NULL,
    b_id            TEXT NOT NULL,
    established_at  INTEGER NOT NULL,
    PRIMARY KEY (a_id, b_id),
    CHECK (a_id < b_id)
);

CREATE INDEX IF NOT EXISTS idx_friendships_b ON friendships(b_id);

-- Direct messages — store-and-forward for the case where the recipient
-- isn't currently online to relay over the mesh. Bounded retention: a
-- scheduled prune drops messages older than 7 days.
CREATE TABLE IF NOT EXISTS direct_messages (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id   TEXT NOT NULL,
    to_id     TEXT NOT NULL,
    body      TEXT NOT NULL,
    sent_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dm_to_sent ON direct_messages(to_id, sent_at);

-- Global chat — IRC-style room visible to every Abyss user, rate-limited
-- server-side. Same 7-day retention.
CREATE TABLE IF NOT EXISTS global_chat (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id   TEXT NOT NULL,
    handle    TEXT NOT NULL,
    body      TEXT NOT NULL,
    sent_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_sent ON global_chat(sent_at);

-- Simple rate-limit token bucket per user_id. Tracks last-N-action
-- timestamps; the worker enforces caps in code (rate-limit logic is
-- easier to express in TS than SQL).
CREATE TABLE IF NOT EXISTS rate_limits (
    user_id     TEXT NOT NULL,
    action      TEXT NOT NULL,                 -- e.g. 'global_chat', 'friend_request'
    last_action INTEGER NOT NULL,
    count       INTEGER NOT NULL,
    PRIMARY KEY (user_id, action)
);

-- A small block list — `blocker` doesn't want to see anything from `blocked`.
CREATE TABLE IF NOT EXISTS blocks (
    blocker     TEXT NOT NULL,
    blocked     TEXT NOT NULL,
    blocked_at  INTEGER NOT NULL,
    PRIMARY KEY (blocker, blocked)
);
