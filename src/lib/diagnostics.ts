/**
 * Typed bridge to the `diagnostics` Rust module — one-button repair flow.
 */

import { invoke } from "@tauri-apps/api/core";

export type CheckStatus = "ok" | "repaired" | "needs_user" | "failed" | "skipped";

export interface CheckResult {
  id:          string;
  title:       string;
  status:      CheckStatus;
  message:     string;
  actionPath?: string;
  actionUrl?:  string;
}

export interface DiagnosticsReport {
  checks:           CheckResult[];
  elapsedMs:        number;
  repairedCount:    number;
  needsUserCount:   number;
  failedCount:      number;
}

export const diagnosticsRunAll = () => invoke<DiagnosticsReport>("diagnostics_run_all");
