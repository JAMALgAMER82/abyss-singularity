/**
 * Couch / big-picture mode.
 *
 * Persisted in localStorage so the user's preference survives a relaunch.
 * The toggle does two things:
 *   1. Adds `abyss-couch` to <html> — CSS in app.css scales the whole UI
 *      up and turns on focus-ring highlights.
 *   2. Activates the `CouchNavigator` component, which polls gamepad input
 *      at 60 Hz and translates D-pad / stick / button events into focus
 *      moves + clicks on whatever element is currently focused.
 */

import { createContext, useContext } from "react";

export interface CouchCtx {
  couch:  boolean;
  toggle: () => void;
  set:    (on: boolean) => void;
}

export const CouchContext = createContext<CouchCtx>({
  couch: false,
  toggle: () => {},
  set:   () => {},
});

export const useCouch = () => useContext(CouchContext);

const STORAGE_KEY = "abyss.couch.enabled";

export function loadCouchPreference(): boolean {
  try { return localStorage.getItem(STORAGE_KEY) === "1"; }
  catch { return false; }
}

export function saveCouchPreference(on: boolean): void {
  try { localStorage.setItem(STORAGE_KEY, on ? "1" : "0"); } catch { /* private mode, ignore */ }
}
