/**
 * Typed bridge to the Rust `transfer` module (Phase 9).
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Platform } from "./library";

export type Direction = "outgoing" | "incoming";

export interface PendingOffer {
  transfer_id: string;
  peer_addr:   string;
  direction:   Direction;
  file_name:   string;
  file_size:   number;
  platform:    Platform;
  sha256:      string;
  offered_at:  string;
}

export type TransferEvent =
  | { kind: "offered";   offer: PendingOffer }
  | { kind: "accepted";  transfer_id: string; peer_addr: string }
  | { kind: "rejected";  transfer_id: string; peer_addr: string }
  | { kind: "started";   transfer_id: string; direction: Direction }
  | { kind: "progress";  transfer_id: string; bytes_done: number; bytes_total: number }
  | { kind: "completed"; transfer_id: string; final_path: string | null; sha256_ok: boolean }
  | { kind: "failed";    transfer_id: string; message: string };

export const TRANSFER_EVENT = "abyss://transfer/event";

export const transferSend          = (entryId: string, peerAddr: string) =>
  invoke<string>("transfer_send", { entryId, peerAddr });
export const transferAccept        = (transferId: string) =>
  invoke<void>("transfer_accept", { transferId });
export const transferReject        = (transferId: string) =>
  invoke<void>("transfer_reject", { transferId });
export const transferListIncoming  = () =>
  invoke<PendingOffer[]>("transfer_list_incoming");

export function onTransferEvent(handler: (e: TransferEvent) => void): Promise<UnlistenFn> {
  return listen<TransferEvent>(TRANSFER_EVENT, (event) => handler(event.payload));
}
