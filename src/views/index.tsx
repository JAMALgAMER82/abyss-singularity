import type { NavId } from "../lib/nav";
import { FriendsView } from "./FriendsView";
import { LibraryView } from "./LibraryView";
import { NetworkView } from "./NetworkView";
import { SettingsView } from "./SettingsView";
import { StreamView } from "./StreamView";

/**
 * View registry. Every nav target now resolves to a real view; the chat
 * data-channel transport for Friends ships in Phase 6.x.
 */
export const VIEWS: Record<NavId, React.ReactElement> = {
  library:  <LibraryView />,
  settings: <SettingsView />,
  network:  <NetworkView />,
  stream:   <StreamView />,
  friends:  <FriendsView />,
};
