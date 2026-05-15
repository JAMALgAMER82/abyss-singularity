/**
 * Abyss Singularity — directory service (Cloudflare Worker).
 *
 * Stateless HTTP front-end over a D1 (SQLite) database. Every Abyss
 * install hits this Worker for:
 *   - presence heartbeats (POST /v1/presence)
 *   - global online list (GET /v1/online)
 *   - friend requests / inbox / accept / reject (POST /v1/friend-*)
 *   - direct messages and global chat (POST/GET /v1/dm, /v1/global-chat)
 *   - per-user privacy: hide (appear offline), block another user
 *
 * Wire format: JSON in + JSON out. No auth — we treat the user_id UUID
 * as a bearer of identity; impersonation is bounded by the rate limits
 * below. For a personal friend-group directory this is acceptable; if
 * you ever want strict identity, swap in a signed-claim flow.
 */

export interface Env {
    DB: D1Database;
}

// ---- helpers ---------------------------------------------------------------

function json(body: unknown, status = 200): Response {
    return new Response(JSON.stringify(body), {
        status,
        headers: {
            "content-type": "application/json",
            "access-control-allow-origin":  "*",
            "access-control-allow-methods": "GET, POST, OPTIONS",
            "access-control-allow-headers": "content-type",
        },
    });
}

function err(message: string, status = 400): Response {
    return json({ ok: false, error: message }, status);
}

function nowMs(): number {
    return Date.now();
}

function assertString(v: unknown, label: string, max = 256): string {
    if (typeof v !== "string") throw new Error(`${label} must be a string`);
    const s = v.trim();
    if (!s)            throw new Error(`${label} is empty`);
    if (s.length > max) throw new Error(`${label} exceeds ${max} chars`);
    return s;
}

function isUuidish(s: string): boolean {
    return /^[0-9a-fA-F-]{16,64}$/.test(s);
}

// Per-user / per-action rate limit. Returns true if the action is
// allowed; false if rate-limited. Cap and window are caller-defined so
// presence can be permissive while friend-requests are strict.
async function rateLimit(
    db:     D1Database,
    userId: string,
    action: string,
    cap:    number,        // max actions allowed within window
    window: number,        // window in ms
): Promise<boolean> {
    const cutoff = nowMs() - window;
    const row = await db
        .prepare("SELECT last_action, count FROM rate_limits WHERE user_id = ? AND action = ?")
        .bind(userId, action)
        .first<{ last_action: number; count: number }>();
    if (!row || row.last_action < cutoff) {
        await db
            .prepare(`
                INSERT INTO rate_limits (user_id, action, last_action, count)
                VALUES (?, ?, ?, 1)
                ON CONFLICT(user_id, action) DO UPDATE
                    SET last_action = excluded.last_action, count = 1
            `)
            .bind(userId, action, nowMs())
            .run();
        return true;
    }
    if (row.count + 1 > cap) return false;
    await db
        .prepare("UPDATE rate_limits SET last_action = ?, count = count + 1 WHERE user_id = ? AND action = ?")
        .bind(nowMs(), userId, action)
        .run();
    return true;
}

// ---- endpoints -------------------------------------------------------------

async function presence(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const id          = assertString(body.user_id, "user_id", 64);
    if (!isUuidish(id)) return err("user_id is not a UUID-ish string");
    const handle      = assertString(body.handle, "handle", 32);
    const appVersion  = assertString(body.app_version, "app_version", 32);
    const country     = typeof body.country === "string" && body.country.length <= 4
                          ? body.country : null;

    if (!await rateLimit(env.DB, id, "presence", 20, 60_000)) {
        return err("presence rate limit exceeded", 429);
    }

    const now = nowMs();
    await env.DB
        .prepare(`
            INSERT INTO users (id, handle, app_version, country, created_at, last_seen)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                handle      = excluded.handle,
                app_version = excluded.app_version,
                country     = excluded.country,
                last_seen   = excluded.last_seen
        `)
        .bind(id, handle, appVersion, country, now, now)
        .run();
    return json({ ok: true, server_time: now });
}

async function online(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const since = Math.min(
        Number(url.searchParams.get("since_ms") ?? "300000") || 300_000,
        24 * 60 * 60_000,  // 24h cap
    );
    const viewer = url.searchParams.get("viewer_id");
    const cutoff = nowMs() - since;
    // Exclude users blocked by viewer (so blocked accounts disappear from
    // their list) and users in 'appear offline' mode.
    const rs = await env.DB
        .prepare(`
            SELECT u.id, u.handle, u.country, u.last_seen, u.app_version
            FROM users u
            WHERE u.last_seen > ?
              AND u.hidden = 0
              AND (? IS NULL OR u.id NOT IN (SELECT blocked FROM blocks WHERE blocker = ?))
            ORDER BY u.last_seen DESC
            LIMIT 200
        `)
        .bind(cutoff, viewer, viewer)
        .all();
    return json({ ok: true, users: rs.results });
}

async function setHidden(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const id = assertString(body.user_id, "user_id", 64);
    const hidden = body.hidden === true ? 1 : 0;
    await env.DB
        .prepare("UPDATE users SET hidden = ? WHERE id = ?")
        .bind(hidden, id)
        .run();
    return json({ ok: true });
}

async function friendRequest(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const fromId      = assertString(body.from_id, "from_id", 64);
    const fromHandle  = assertString(body.from_handle, "from_handle", 32);
    const toId        = assertString(body.to_id, "to_id", 64);
    // `invite_code` is optional — directory-only friendships work without
    // ever swapping tailnets. The sender can include their auth key if
    // they also want to mesh-pair on accept, but it's not required.
    const inviteCode  = typeof body.invite_code === "string" && body.invite_code.length > 0
                          ? body.invite_code : null;
    const message     = typeof body.message === "string" && body.message.length <= 280
                          ? body.message : null;

    if (fromId === toId) return err("cannot friend yourself");

    if (!await rateLimit(env.DB, fromId, "friend_request", 20, 60 * 60_000)) {
        return err("friend-request rate limit exceeded (20/hour)", 429);
    }

    // If a pending request already exists, surface it instead of duplicating.
    const existing = await env.DB
        .prepare("SELECT id, status FROM friend_requests WHERE from_id = ? AND to_id = ? AND status = 'pending'")
        .bind(fromId, toId)
        .first<{ id: number; status: string }>();
    if (existing) {
        return json({ ok: true, request_id: existing.id, dedup: true });
    }

    const r = await env.DB
        .prepare(`
            INSERT INTO friend_requests (from_id, to_id, from_handle, message, invite_code, status, created_at)
            VALUES (?, ?, ?, ?, ?, 'pending', ?)
        `)
        .bind(fromId, toId, fromHandle, message, inviteCode, nowMs())
        .run();
    return json({ ok: true, request_id: r.meta.last_row_id });
}

async function friendRequestInbox(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const userId = assertString(url.searchParams.get("user_id"), "user_id", 64);
    const rs = await env.DB
        .prepare(`
            SELECT id, from_id, from_handle, message, invite_code, created_at
            FROM friend_requests
            WHERE to_id = ? AND status = 'pending'
            ORDER BY created_at DESC
            LIMIT 50
        `)
        .bind(userId)
        .all();
    return json({ ok: true, requests: rs.results });
}

async function friendRequestSent(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const userId = assertString(url.searchParams.get("user_id"), "user_id", 64);
    // Surface acceptances so the sender can fetch the recipient's invite
    // code and join their tailnet too — symmetric peering.
    const rs = await env.DB
        .prepare(`
            SELECT id, to_id, status, accept_invite_code, responded_at, created_at
            FROM friend_requests
            WHERE from_id = ? AND status IN ('accepted', 'rejected')
                AND responded_at > ?
            ORDER BY responded_at DESC
            LIMIT 50
        `)
        .bind(userId, nowMs() - 24 * 60 * 60_000)
        .all();
    return json({ ok: true, responses: rs.results });
}

async function friendAccept(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const requestId    = Number(body.request_id);
    if (!Number.isFinite(requestId)) return err("request_id must be a number");
    const userId       = assertString(body.user_id, "user_id", 64);
    // Acceptance can include an auth key for mutual mesh-pairing, but
    // it's optional — pure directory friendships are fine without one.
    const inviteCode   = typeof body.invite_code === "string" && body.invite_code.length > 0
                          ? body.invite_code : null;

    const fr = await env.DB
        .prepare("SELECT * FROM friend_requests WHERE id = ?")
        .bind(requestId)
        .first<{ id: number; from_id: string; to_id: string; status: string }>();
    if (!fr)              return err("request not found", 404);
    if (fr.to_id !== userId) return err("not your request", 403);
    if (fr.status !== "pending") return err(`already ${fr.status}`, 409);

    const now = nowMs();
    // Two writes in one transaction so we don't leave half-state on partial failure.
    const aId = fr.from_id < fr.to_id ? fr.from_id : fr.to_id;
    const bId = fr.from_id < fr.to_id ? fr.to_id   : fr.from_id;
    await env.DB.batch([
        env.DB.prepare("UPDATE friend_requests SET status='accepted', responded_at=?, accept_invite_code=? WHERE id=?")
            .bind(now, inviteCode, requestId),
        env.DB.prepare("INSERT OR IGNORE INTO friendships (a_id, b_id, established_at) VALUES (?, ?, ?)")
            .bind(aId, bId, now),
    ]);
    return json({ ok: true });
}

async function friendReject(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const requestId = Number(body.request_id);
    if (!Number.isFinite(requestId)) return err("request_id must be a number");
    const userId = assertString(body.user_id, "user_id", 64);
    const fr = await env.DB
        .prepare("SELECT to_id, status FROM friend_requests WHERE id = ?")
        .bind(requestId)
        .first<{ to_id: string; status: string }>();
    if (!fr)                     return err("request not found", 404);
    if (fr.to_id !== userId)      return err("not your request", 403);
    if (fr.status !== "pending") return err(`already ${fr.status}`, 409);

    await env.DB
        .prepare("UPDATE friend_requests SET status='rejected', responded_at=? WHERE id=?")
        .bind(nowMs(), requestId)
        .run();
    return json({ ok: true });
}

async function friends(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const userId = assertString(url.searchParams.get("user_id"), "user_id", 64);
    // Join friendships → users for handle + last_seen, with friend on either side.
    const rs = await env.DB
        .prepare(`
            SELECT u.id, u.handle, u.country, u.last_seen, u.hidden, f.established_at
            FROM friendships f
            JOIN users u
              ON u.id = CASE WHEN f.a_id = ? THEN f.b_id ELSE f.a_id END
            WHERE f.a_id = ? OR f.b_id = ?
            ORDER BY u.last_seen DESC
            LIMIT 200
        `)
        .bind(userId, userId, userId)
        .all();
    return json({ ok: true, friends: rs.results });
}

async function sendDm(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const fromId = assertString(body.from_id, "from_id", 64);
    const toId   = assertString(body.to_id, "to_id", 64);
    const text   = assertString(body.body, "body", 2000);

    if (!await rateLimit(env.DB, fromId, "dm", 60, 60_000)) {
        return err("DM rate limit exceeded (60/min)", 429);
    }

    // Recipient must have blocked check.
    const blocked = await env.DB
        .prepare("SELECT 1 FROM blocks WHERE blocker = ? AND blocked = ? LIMIT 1")
        .bind(toId, fromId)
        .first();
    if (blocked) {
        // Accept silently — don't reveal block to sender.
        return json({ ok: true, delivered: false });
    }

    const r = await env.DB
        .prepare("INSERT INTO direct_messages (from_id, to_id, body, sent_at) VALUES (?, ?, ?, ?)")
        .bind(fromId, toId, text, nowMs())
        .run();
    return json({ ok: true, message_id: r.meta.last_row_id });
}

async function getDms(req: Request, env: Env): Promise<Response> {
    const url    = new URL(req.url);
    const userId = assertString(url.searchParams.get("user_id"), "user_id", 64);
    const since  = Number(url.searchParams.get("since_ms") ?? "86400000") || 86_400_000;
    const cutoff = nowMs() - Math.min(since, 7 * 24 * 60 * 60_000);
    // Return both incoming and outgoing so the UI can render a conversation thread.
    const rs = await env.DB
        .prepare(`
            SELECT id, from_id, to_id, body, sent_at FROM direct_messages
            WHERE (from_id = ? OR to_id = ?) AND sent_at > ?
            ORDER BY sent_at ASC
            LIMIT 500
        `)
        .bind(userId, userId, cutoff)
        .all();
    return json({ ok: true, messages: rs.results });
}

async function globalChat(req: Request, env: Env): Promise<Response> {
    if (req.method === "GET") {
        const url   = new URL(req.url);
        const since = Number(url.searchParams.get("since_ms") ?? "3600000") || 3_600_000;
        const cutoff = nowMs() - Math.min(since, 24 * 60 * 60_000);
        const rs = await env.DB
            .prepare("SELECT id, user_id, handle, body, sent_at FROM global_chat WHERE sent_at > ? ORDER BY sent_at ASC LIMIT 200")
            .bind(cutoff).all();
        return json({ ok: true, messages: rs.results });
    }
    // POST
    const body = await req.json() as Record<string, unknown>;
    const userId = assertString(body.user_id, "user_id", 64);
    const handle = assertString(body.handle, "handle", 32);
    const text   = assertString(body.body, "body", 500);
    if (!await rateLimit(env.DB, userId, "global_chat", 6, 60_000)) {
        return err("global-chat rate limit exceeded (6/min)", 429);
    }
    const r = await env.DB
        .prepare("INSERT INTO global_chat (user_id, handle, body, sent_at) VALUES (?, ?, ?, ?)")
        .bind(userId, handle, text, nowMs())
        .run();
    return json({ ok: true, message_id: r.meta.last_row_id });
}

async function block(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const blocker = assertString(body.user_id, "user_id", 64);
    const blocked = assertString(body.target_id, "target_id", 64);
    if (blocker === blocked) return err("cannot block yourself");
    await env.DB
        .prepare("INSERT OR IGNORE INTO blocks (blocker, blocked, blocked_at) VALUES (?, ?, ?)")
        .bind(blocker, blocked, nowMs())
        .run();
    return json({ ok: true });
}

async function unblock(req: Request, env: Env): Promise<Response> {
    const body = await req.json() as Record<string, unknown>;
    const blocker = assertString(body.user_id, "user_id", 64);
    const blocked = assertString(body.target_id, "target_id", 64);
    await env.DB
        .prepare("DELETE FROM blocks WHERE blocker = ? AND blocked = ?")
        .bind(blocker, blocked)
        .run();
    return json({ ok: true });
}

// ---- router ----------------------------------------------------------------

export default {
    async fetch(req: Request, env: Env): Promise<Response> {
        // CORS preflight — Tauri calls from the desktop bypass this, but
        // local dev (Vite at localhost:1420) hits the same Worker.
        if (req.method === "OPTIONS") return json({ ok: true });

        const url   = new URL(req.url);
        const route = `${req.method} ${url.pathname}`;
        try {
            // NOTE: single-space between method + path. `${m} ${p}`
            // produces "GET /v1/online" not "GET  /v1/online".
            switch (route) {
                case "POST /v1/presence":         return await presence(req, env);
                case "GET /v1/online":            return await online(req, env);
                case "POST /v1/hidden":           return await setHidden(req, env);
                case "POST /v1/friend-request":   return await friendRequest(req, env);
                case "GET /v1/friend-requests":   return await friendRequestInbox(req, env);
                case "GET /v1/friend-responses":  return await friendRequestSent(req, env);
                case "POST /v1/friend-accept":    return await friendAccept(req, env);
                case "POST /v1/friend-reject":    return await friendReject(req, env);
                case "GET /v1/friends":           return await friends(req, env);
                case "POST /v1/dm":               return await sendDm(req, env);
                case "GET /v1/dm":                return await getDms(req, env);
                case "GET /v1/global-chat":
                case "POST /v1/global-chat":      return await globalChat(req, env);
                case "POST /v1/block":            return await block(req, env);
                case "POST /v1/unblock":          return await unblock(req, env);
                case "GET /":
                case "GET /health":               return json({ ok: true, service: "abyss-directory", now: nowMs() });
            }
            return err("not found", 404);
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            return err(msg, 400);
        }
    },

    // Daily cleanup — drop ancient DMs / chat / stale rate-limit rows so
    // the database doesn't grow unbounded. Bound to a Cron Trigger in
    // wrangler.toml.
    async scheduled(_evt: ScheduledEvent, env: Env, _ctx: ExecutionContext): Promise<void> {
        const dmCutoff   = nowMs() - 7  * 24 * 60 * 60_000;
        const chatCutoff = nowMs() - 30 * 24 * 60 * 60_000;
        const rlCutoff   = nowMs() - 24 * 60 * 60_000;
        await env.DB.batch([
            env.DB.prepare("DELETE FROM direct_messages WHERE sent_at < ?").bind(dmCutoff),
            env.DB.prepare("DELETE FROM global_chat     WHERE sent_at < ?").bind(chatCutoff),
            env.DB.prepare("DELETE FROM rate_limits     WHERE last_action < ?").bind(rlCutoff),
        ]);
    },
} satisfies ExportedHandler<Env>;
