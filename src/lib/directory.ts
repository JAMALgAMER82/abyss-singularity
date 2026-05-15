/**
 * Typed bridge to the Rust `directory` module (Phase 14).
 *
 * The directory is Abyss's global presence layer: who's online globally,
 * friend list, friend requests, DMs, global chat. Backed by a tiny
 * Cloudflare Worker the user deploys once (see `abyss-directory/` in
 * the repo). Decoupled from the Tailscale mesh — directory friendships
 * are social-only by default; mesh peering is a separate opt-in.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- types (must match src-tauri/src/directory/types.rs) ------------------

export interface DirectoryConfig {
  user_id:    string | null;
  handle:     string | null;
  worker_url: string | null;
  hidden:     boolean;
  country:    string | null;
}

export interface OnlineUser {
  id:          string;
  handle:      string;
  country:     string | null;
  last_seen:   number;        // epoch ms
  app_version: string;
}

export interface FriendRequest {
  id:          number;
  from_id:     string;
  from_handle: string;
  message:     string | null;
  invite_code: string | null;
  created_at:  number;
}

export interface FriendResponse {
  id:                 number;
  to_id:              string;
  status:             "accepted" | "rejected";
  accept_invite_code: string | null;
  responded_at:       number | null;
  created_at:         number;
}

export interface Friend {
  id:             string;
  handle:         string;
  country:        string | null;
  last_seen:      number;
  hidden:         number;          // SQLite stores 0/1
  established_at: number;
}

export interface DirectMessage {
  id:      number;
  from_id: string;
  to_id:   string;
  body:    string;
  sent_at: number;
}

export interface GlobalChatMessage {
  id:      number;
  user_id: string;
  handle:  string;
  body:    string;
  sent_at: number;
}

// ---- command wrappers -----------------------------------------------------

export const dirGetConfig = () => invoke<DirectoryConfig>("dir_get_config");
export const dirSetConfig = (patch: {
  handle?:     string;
  workerUrl?:  string;
  hidden?:     boolean;
  country?:    string;
}) => invoke<DirectoryConfig>("dir_set_config", {
  handle:     patch.handle    ?? null,
  workerUrl:  patch.workerUrl ?? null,
  hidden:     patch.hidden    ?? null,
  country:    patch.country   ?? null,
});

export const dirOnline = (sinceMs = 300_000) =>
  invoke<OnlineUser[]>("dir_online", { sinceMs });

export const dirSendFriendRequest = (toId: string, inviteCode?: string, message?: string) =>
  invoke<number>("dir_send_friend_request", {
    toId,
    inviteCode: inviteCode ?? null,
    message:    message    ?? null,
  });

export const dirFriendRequests   = () => invoke<FriendRequest[]>("dir_friend_requests");
export const dirFriendResponses  = () => invoke<FriendResponse[]>("dir_friend_responses");

export const dirAcceptRequest = (requestId: number, inviteCode?: string) =>
  invoke<void>("dir_accept_request", { requestId, inviteCode: inviteCode ?? null });
export const dirRejectRequest = (requestId: number) =>
  invoke<void>("dir_reject_request", { requestId });

export const dirFriends = () => invoke<Friend[]>("dir_friends");

export const dirSendDm = (toId: string, body: string) =>
  invoke<number>("dir_send_dm", { toId, body });
export const dirGetDms = (sinceMs = 86_400_000) =>
  invoke<DirectMessage[]>("dir_get_dms", { sinceMs });

export const dirSendGlobalChat = (body: string) =>
  invoke<number>("dir_send_global_chat", { body });
export const dirGetGlobalChat = (sinceMs = 3_600_000) =>
  invoke<GlobalChatMessage[]>("dir_get_global_chat", { sinceMs });

export const dirBlock   = (targetId: string) => invoke<void>("dir_block",   { targetId });
export const dirUnblock = (targetId: string) => invoke<void>("dir_unblock", { targetId });

// ---- live events ----------------------------------------------------------

export async function onDirFriendRequest(cb: (rs: FriendRequest[]) => void): Promise<UnlistenFn> {
  return listen<FriendRequest[]>("abyss://directory/friend-request", (e) => cb(e.payload));
}
export async function onDirFriendResponse(cb: (rs: FriendResponse[]) => void): Promise<UnlistenFn> {
  return listen<FriendResponse[]>("abyss://directory/friend-response", (e) => cb(e.payload));
}
export async function onDirDm(cb: (ms: DirectMessage[]) => void): Promise<UnlistenFn> {
  return listen<DirectMessage[]>("abyss://directory/dm", (e) => cb(e.payload));
}
