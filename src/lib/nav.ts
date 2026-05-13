/**
 * Navigation registry — the single source of truth for which top-level
 * "views" exist in the Abyss shell. Adding a new section is just adding
 * an entry here plus a component in src/views.
 */

export type NavId =
  | "library"
  | "network"
  | "stream"
  | "friends"
  | "settings";

export interface NavItem {
  id: NavId;
  label: string;
  hotkey?: string;
  /** Phase number that wires up this view's real functionality. */
  phase: number;
}

export const NAV_ITEMS: readonly NavItem[] = [
  { id: "library",  label: "Library",  hotkey: "1", phase: 2 },
  { id: "network",  label: "Network",  hotkey: "2", phase: 4 },
  { id: "stream",   label: "Stream",   hotkey: "3", phase: 5 },
  { id: "friends",  label: "Friends",  hotkey: "4", phase: 6 },
  { id: "settings", label: "Settings", hotkey: "5", phase: 1 },
] as const;
