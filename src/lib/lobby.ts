/**
 * Typed bridge to the Rust `lobby` module (Phase 12).
 *
 * The lobby builds on top of the existing chat layer — a "room" is just
 * a piece of state the host broadcasts via `ChatProtocol::LobbyAdvertise`
 * to every connected peer. When the host hits Start, the same channel
 * carries a `LobbyStartGame` frame; each member's Abyss launches their
 * local copy of the same game as a RetroArch netplay client.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Platform } from "./library";

export type RoomRole = "host" | "member";

export interface RoomMember {
  addr:         string;
  display_name: string | null;
}

export interface RoomSnapshot {
  role:       RoomRole | null;
  host_addr:  string | null;
  host_name:  string | null;
  platform:   Platform | null;
  game_name:  string | null;
  members:    RoomMember[];
}

export interface LobbyLaunchReport {
  run_id:       string;
  command_line: string;
  role:         RoomRole;
}

/** Fired whenever our local room snapshot changes. */
export interface LobbyStateEvent extends RoomSnapshot {}

/** Fired when some other peer broadcasts they're hosting a room we're not in. */
export interface LobbyIncomingInvite {
  host_addr: string;
  host_name: string;
  platform:  Platform;
  game_name: string;
  members:   string[];
}

export const lobbyState        = () => invoke<RoomSnapshot>("lobby_state");
export const lobbyHostRoom     = (platform: Platform, gameName: string) =>
  invoke<RoomSnapshot>("lobby_host_room", { platform, gameName });
export const lobbyCloseRoom    = () => invoke<RoomSnapshot>("lobby_close_room");
export const lobbyRequestJoin  = (hostAddr: string) =>
  invoke<void>("lobby_request_join", { hostAddr });
export const lobbyLeaveRoom    = () => invoke<RoomSnapshot>("lobby_leave_room");
export const lobbyStartGame    = (hostIp: string) =>
  invoke<LobbyLaunchReport>("lobby_start_game", { hostIp });

export async function onLobbyState(cb: (state: LobbyStateEvent) => void): Promise<UnlistenFn> {
  return listen<LobbyStateEvent>("abyss://lobby/state", (e) => cb(e.payload));
}

export async function onLobbyIncomingInvite(cb: (i: LobbyIncomingInvite) => void): Promise<UnlistenFn> {
  return listen<LobbyIncomingInvite>("abyss://lobby/incoming-invite", (e) => cb(e.payload));
}
