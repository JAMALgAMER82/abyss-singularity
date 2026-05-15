/**
 * Inline icon set. Kept dependency-free for Phase 1 — once `lucide-react`
 * is installed in Phase 1b, individual call sites can migrate without any
 * refactor since the prop shape (`size`, `className`) matches lucide's.
 */

import type { ReactElement, SVGProps } from "react";
import type { NavId } from "../lib/nav";

type IconProps = SVGProps<SVGSVGElement> & { size?: number };

const baseProps = (p: IconProps) => ({
  width: p.size ?? 18,
  height: p.size ?? 18,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.75,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  ...p,
});

export const LibraryIcon  = (p: IconProps) => (
  <svg {...baseProps(p)}>
    <rect x="3"  y="4" width="4" height="16" rx="1" />
    <rect x="9"  y="4" width="4" height="16" rx="1" />
    <path d="M16 4l4 1-3 15-4-1z" />
  </svg>
);
export const NetworkIcon  = (p: IconProps) => (
  <svg {...baseProps(p)}>
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 010 18M12 3a14 14 0 000 18" />
  </svg>
);
export const StreamIcon   = (p: IconProps) => (
  <svg {...baseProps(p)}>
    <rect x="2" y="5" width="20" height="14" rx="2" />
    <path d="M10 9l5 3-5 3z" fill="currentColor" stroke="none" />
  </svg>
);
export const FriendsIcon  = (p: IconProps) => (
  <svg {...baseProps(p)}>
    <circle cx="9"  cy="9" r="3.5" />
    <circle cx="17" cy="10" r="2.5" />
    <path d="M3 19c0-3 2.5-5 6-5s6 2 6 5" />
    <path d="M15 19c0-2 1.5-3.5 4-3.5s2 0 2 0" />
  </svg>
);
export const DiscoverIcon = (p: IconProps) => (
  <svg {...baseProps(p)}>
    <circle cx="12" cy="12" r="9" />
    <path d="M9 12l4 -4 -2 6 -4 4 2 -6z" fill="currentColor" stroke="none" />
  </svg>
);
export const SettingsIcon = (p: IconProps) => (
  <svg {...baseProps(p)}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3h0a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8v0a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z" />
  </svg>
);

export const MinimizeIcon = (p: IconProps) => (
  <svg {...baseProps(p)}><path d="M5 12h14" /></svg>
);
export const MaximizeIcon = (p: IconProps) => (
  <svg {...baseProps(p)}><rect x="5" y="5" width="14" height="14" rx="1" /></svg>
);
export const CloseIcon = (p: IconProps) => (
  <svg {...baseProps(p)}><path d="M6 6l12 12M18 6L6 18" /></svg>
);

export const NAV_ICONS: Record<NavId, (p: IconProps) => ReactElement> = {
  library:  LibraryIcon,
  network:  NetworkIcon,
  stream:   StreamIcon,
  friends:  FriendsIcon,
  discover: DiscoverIcon,
  settings: SettingsIcon,
};
