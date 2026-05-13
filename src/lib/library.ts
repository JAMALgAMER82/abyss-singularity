/**
 * Typed bridge to the Rust `library` module.
 *
 * Every export here is a thin wrapper over `invoke()`/`listen()` that
 * gives the rest of the React code real types instead of `unknown`.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- types (must match src-tauri/src/library/types.rs) --------------------

export type Platform =
  | "pc" | "nes" | "snes" | "n64"
  | "game_cube" | "wii" | "wii_u" | "switch"
  | "gameboy" | "gameboy_color" | "gameboy_advance"
  | "nds" | "threeds"
  | "ps1" | "ps2" | "ps3" | "psp" | "ps_vita"
  | "genesis" | "master_system" | "game_gear"
  | "saturn" | "dreamcast"
  | "atari2600" | "neo_geo" | "arcade"
  | "other";

export const PLATFORM_DISPLAY: Record<Platform, string> = {
  pc:               "PC",
  nes:              "NES",
  snes:             "SNES",
  n64:              "Nintendo 64",
  game_cube:        "GameCube",
  wii:              "Wii",
  wii_u:            "Wii U",
  switch:           "Switch",
  gameboy:          "Game Boy",
  gameboy_color:    "Game Boy Color",
  gameboy_advance:  "Game Boy Advance",
  nds:              "Nintendo DS",
  threeds:          "Nintendo 3DS",
  ps1:              "PlayStation",
  ps2:              "PlayStation 2",
  ps3:              "PlayStation 3",
  psp:              "PSP",
  ps_vita:          "PS Vita",
  genesis:          "Mega Drive / Genesis",
  master_system:    "Master System",
  game_gear:        "Game Gear",
  saturn:           "Saturn",
  dreamcast:        "Dreamcast",
  atari2600:        "Atari 2600",
  neo_geo:          "Neo Geo",
  arcade:           "Arcade",
  other:            "Other",
};

export interface IgdbMetadata {
  igdb_id: number;
  name: string;
  summary?: string | null;
  cover_url?: string | null;
  release_year?: number | null;
  total_rating?: number | null;
  platforms: string[];
}

export interface LibraryEntry {
  id: string;
  path: string;
  file_name: string;
  stem: string;
  extension: string;
  size_bytes: number;
  modified: string;       // ISO datetime
  platform: Platform;
  igdb?: IgdbMetadata | null;
  cover_local_path?: string | null;
  last_enriched?: string | null;
}

export interface ScanReport {
  roots: string[];
  total_files_seen: number;
  games_found: number;
  games_new: number;
  games_kept: number;
  elapsed_ms: number;
}

export interface ScanProgressEvent {
  root: string;
  files_seen: number;
  games_found: number;
  current_file: string | null;
}

export interface ScanResult {
  report: ScanReport;
  entries: LibraryEntry[];
}

export interface LibraryConfig {
  scan_paths: string[];
  igdb_client_id?: string | null;
  igdb_client_secret?: string | null;
  wizard_completed_at?: string | null;
}

// ---- enrichment types ----------------------------------------------------

export interface EnrichProgressEvent {
  processed: number;
  total: number;
  matched: number;
  current: string | null;
}

export interface EnrichReport {
  processed: number;
  matched: number;
  skipped: number;
  errors: number;
  elapsed_ms: number;
  entries: LibraryEntry[];
}

// ---- command wrappers ----------------------------------------------------

export const SCAN_PROGRESS_EVENT    = "abyss://library/scan-progress";
export const ENRICH_PROGRESS_EVENT  = "abyss://library/enrich-progress";

export const getConfig             = () => invoke<LibraryConfig>("library_get_config");
export const setConfig             = (config: LibraryConfig) => invoke<void>("library_set_config", { config });
export const addPath               = (path: string) => invoke<LibraryConfig>("library_add_path", { path });
export const removePath            = (path: string) => invoke<LibraryConfig>("library_remove_path", { path });
export const loadLibrary           = () => invoke<LibraryEntry[]>("library_load");
export const scanLibrary           = () => invoke<ScanResult>("library_scan");
export const setIgdbCredentials    = (clientId: string, clientSecret: string) =>
  invoke<void>("library_set_igdb_credentials", { clientId, clientSecret });
export const enrichLibraryMetadata = (force = false) =>
  invoke<EnrichReport>("library_enrich_metadata", { force });

export function onScanProgress(handler: (e: ScanProgressEvent) => void): Promise<UnlistenFn> {
  return listen<ScanProgressEvent>(SCAN_PROGRESS_EVENT, (event) => handler(event.payload));
}
export function onEnrichProgress(handler: (e: EnrichProgressEvent) => void): Promise<UnlistenFn> {
  return listen<EnrichProgressEvent>(ENRICH_PROGRESS_EVENT, (event) => handler(event.payload));
}
