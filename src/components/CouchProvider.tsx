import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CouchContext,
  loadCouchPreference,
  saveCouchPreference,
} from "../lib/couch";

interface CouchProviderProps {
  children: React.ReactNode;
}

export function CouchProvider({ children }: CouchProviderProps) {
  const [couch, setCouch] = useState<boolean>(() => loadCouchPreference());

  // Body class drives the giant-text CSS variant.
  useEffect(() => {
    const root = document.documentElement;
    if (couch) root.classList.add("abyss-couch");
    else       root.classList.remove("abyss-couch");
    saveCouchPreference(couch);
  }, [couch]);

  const toggle = useCallback(() => setCouch((c) => !c), []);
  const set    = useCallback((on: boolean) => setCouch(on), []);

  // Gamepad **Start** button (index 9 in the standard mapping) toggles
  // couch mode globally — same convention the rest of the UI uses for
  // big-picture launchers. We poll lightly when *off* (the only thing we
  // care about is the press); the heavy 60 Hz nav loop only runs when
  // already in couch mode.
  useEffect(() => {
    let prev = false;
    let raf: number | null = null;
    const tick = () => {
      const pads = navigator.getGamepads?.() ?? [];
      const start = Array.from(pads).some(
        (p) => p?.connected && p.buttons[9]?.pressed,
      );
      if (start && !prev) setCouch((c) => !c);
      prev = start;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => { if (raf !== null) cancelAnimationFrame(raf); };
  }, []);

  // Keyboard hotkey: F11 also toggles. Doesn't conflict with the OS
  // fullscreen toggle since we own the title bar.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "F11") {
        e.preventDefault();
        setCouch((c) => !c);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const ctx = useMemo(() => ({ couch, toggle, set }), [couch, toggle, set]);

  return (
    <CouchContext.Provider value={ctx}>
      {children}
      {couch && <CouchNavigator />}
    </CouchContext.Provider>
  );
}

/**
 * 60 Hz gamepad → focus/click translator. Only mounted while couch
 * mode is active.
 *
 *   D-pad / Left stick:   move focus (Tab / Shift+Tab)
 *   A button (0):         click the focused element
 *   B button (1):         blur (back out of a control)
 *   Right stick / wheel:  scroll the main pane
 */
function CouchNavigator() {
  useEffect(() => {
    // Edge-detected button + axis state. We only fire on a *transition*
    // from inactive to active, so holding a direction doesn't spam.
    const prev = {
      up: false, down: false, left: false, right: false,
      a: false, b: false,
    };
    const DEADZONE = 0.55;
    // Repeat-key behaviour: once a direction is held past initial press,
    // re-fire every `REPEAT_MS` so the user can scrub across a long list.
    let lastRepeat = 0;
    const REPEAT_MS = 180;

    function navigate(direction: "next" | "prev") {
      // Use Tab navigation — every focusable element in the app
      // (button, input, [tabindex]) participates without per-component
      // wiring. We synthesise the keydown the same way the OS would
      // dispatch one.
      const ev = new KeyboardEvent("keydown", {
        key:       "Tab",
        code:      "Tab",
        keyCode:   9,
        which:     9,
        shiftKey:  direction === "prev",
        bubbles:   true,
        cancelable: true,
      });
      document.dispatchEvent(ev);
      // The above doesn't actually move focus in most browsers; do it
      // manually by walking [tabIndex >= 0] elements.
      moveFocus(direction === "next" ? +1 : -1);
    }

    function clickFocused() {
      const el = document.activeElement as HTMLElement | null;
      if (!el) return;
      // Buttons/links/inputs respond to .click(); for others fall back
      // to a synthetic click event so React onClick handlers fire.
      if (typeof (el as HTMLButtonElement).click === "function") {
        el.click();
      } else {
        el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      }
    }

    function blurFocused() {
      const el = document.activeElement as HTMLElement | null;
      if (el && typeof el.blur === "function") el.blur();
    }

    let raf: number | null = null;
    const tick = () => {
      const pads = navigator.getGamepads?.() ?? [];
      // Combine input across every connected pad — so co-op-on-couch
      // (two pads, one TV) "just works" without per-pad routing.
      let up = false, down = false, left = false, right = false, a = false, b = false;
      for (const p of pads) {
        if (!p?.connected) continue;
        // Buttons 12-15 = dpad up/down/left/right in standard mapping.
        up    ||= p.buttons[12]?.pressed ?? false;
        down  ||= p.buttons[13]?.pressed ?? false;
        left  ||= p.buttons[14]?.pressed ?? false;
        right ||= p.buttons[15]?.pressed ?? false;
        // Left stick fallback.
        const ax = p.axes[0] ?? 0;
        const ay = p.axes[1] ?? 0;
        if (ay < -DEADZONE) up    = true;
        if (ay >  DEADZONE) down  = true;
        if (ax < -DEADZONE) left  = true;
        if (ax >  DEADZONE) right = true;
        a ||= p.buttons[0]?.pressed ?? false;
        b ||= p.buttons[1]?.pressed ?? false;
      }

      const now = performance.now();
      // Edge-trigger nav OR repeat if a direction has been held long enough.
      const hold = up || down || left || right;
      const wasHold = prev.up || prev.down || prev.left || prev.right;
      const shouldFire =
        (hold && !wasHold) ||
        (hold && now - lastRepeat > REPEAT_MS);

      if (shouldFire) {
        if (down || right) navigate("next");
        else if (up || left) navigate("prev");
        lastRepeat = now;
      }
      if (a && !prev.a) clickFocused();
      if (b && !prev.b) blurFocused();

      prev.up = up; prev.down = down; prev.left = left; prev.right = right;
      prev.a = a;  prev.b = b;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => { if (raf !== null) cancelAnimationFrame(raf); };
  }, []);

  // Auto-focus the first focusable element on mount so the user has
  // something to "act on" immediately when they enter couch mode.
  useEffect(() => {
    const first = focusables()[0];
    first?.focus();
  }, []);

  return null;
}

/** Manhattan-tab focus traversal. Walks every focusable element in DOM
 *  order and moves focus by `delta` positions, wrapping around. */
function moveFocus(delta: number) {
  const els = focusables();
  if (els.length === 0) return;
  const current = document.activeElement as HTMLElement | null;
  let idx = current ? els.indexOf(current) : -1;
  if (idx < 0) idx = 0;
  let next = (idx + delta) % els.length;
  if (next < 0) next = els.length + next;
  els[next]?.focus();
}

function focusables(): HTMLElement[] {
  const sel = [
    "a[href]:not([disabled])",
    "button:not([disabled])",
    "input:not([disabled])",
    "select:not([disabled])",
    "textarea:not([disabled])",
    "[tabindex]:not([tabindex='-1']):not([disabled])",
  ].join(",");
  return Array.from(document.querySelectorAll<HTMLElement>(sel))
    .filter((el) => !el.hasAttribute("hidden") && el.offsetParent !== null);
}
