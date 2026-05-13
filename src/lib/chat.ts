/**
 * Typed bridge to the Rust `chat` module (Phase 6.x).
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type PresenceStatus = "idle" | "playing" | "streaming" | "away";
export type Direction      = "inbound" | "outbound";

export interface ChatConfig {
  display_name: string | null;
  listen_port:  number;
  enabled:      boolean;
}

export interface ChatStatus {
  running:   boolean;
  self_name: string;
  presence:  PresenceStatus;
  activity:  string | null;
}

export interface ChatHistoryEntry {
  id:        string;
  peer_addr: string;
  direction: Direction;
  body:      string;
  at:        string;
}

export interface PeerSnapshot {
  addr:         string;
  display_name: string | null;
  connected:    boolean;
  presence:     PresenceStatus | null;
  activity:     string | null;
  last_seen:    string | null;
}

export const CHAT_MESSAGE_EVENT = "abyss://chat/message";
export const CHAT_PEER_EVENT    = "abyss://chat/peer-update";

export const chatGetConfig    = () => invoke<ChatConfig>("chat_get_config");
export const chatSetConfig    = (config: ChatConfig) => invoke<void>("chat_set_config", { config });
export const chatStart        = () => invoke<number>("chat_start");
export const chatStop         = () => invoke<void>("chat_stop");
export const chatStatus       = () => invoke<ChatStatus>("chat_status");
export const chatConnectPeer  = (host: string, port?: number) =>
  invoke<void>("chat_connect_peer", { host, port: port ?? null });
export const chatSend         = (peerAddr: string, body: string) =>
  invoke<ChatHistoryEntry>("chat_send", { peerAddr, body });
export const chatGetHistory   = (peerAddr?: string) =>
  invoke<ChatHistoryEntry[]>("chat_get_history", { peerAddr: peerAddr ?? null });
export const chatGetPeers     = () => invoke<PeerSnapshot[]>("chat_get_peers");
export const chatSetPresence  = (status: PresenceStatus, activity?: string | null) =>
  invoke<void>("chat_set_presence", { status, activity: activity ?? null });

export function onChatMessage(handler: (msg: ChatHistoryEntry) => void): Promise<UnlistenFn> {
  return listen<ChatHistoryEntry>(CHAT_MESSAGE_EVENT, (event) => handler(event.payload));
}
export function onChatPeers(handler: (peers: PeerSnapshot[]) => void): Promise<UnlistenFn> {
  return listen<PeerSnapshot[]>(CHAT_PEER_EVENT, (event) => handler(event.payload));
}
