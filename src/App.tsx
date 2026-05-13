import { useEffect, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { FirstRunWizard } from "./components/FirstRunWizard";
import { GlobalToasts } from "./components/GlobalToasts";
import { CouchProvider } from "./components/CouchProvider";
import { getConfig } from "./lib/library";
import { NAV_ITEMS, type NavId } from "./lib/nav";
import { VIEWS } from "./views";

function App() {
  const [active, setActive] = useState<NavId>("library");
  const [showWizard, setShowWizard] = useState<boolean | null>(null);

  // First-launch detection: show the wizard if the library config has no
  // `wizard_completed_at` timestamp. Persists across restarts.
  useEffect(() => {
    getConfig()
      .then((c) => setShowWizard(!c.wizard_completed_at))
      .catch(() => setShowWizard(false));
  }, []);

  // Ctrl+1..5 hotkeys cycle the primary nav. Cheap to do here; if it grows
  // beyond a handful of bindings we'll lift it into a hook.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const item = NAV_ITEMS.find((i) => i.hotkey === e.key);
      if (item) {
        e.preventDefault();
        setActive(item.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const activeItem = NAV_ITEMS.find((i) => i.id === active)!;

  return (
    <CouchProvider>
      <div className="flex h-screen w-screen flex-col bg-abyss-bg">
        <TitleBar title={activeItem.label} subtitle={`Phase ${activeItem.phase}`} />
        <div className="flex flex-1 overflow-hidden">
          <Sidebar active={active} onSelect={setActive} />
          <main className="flex-1 overflow-auto">{VIEWS[active]}</main>
        </div>
        {showWizard && <FirstRunWizard onDone={() => setShowWizard(false)} />}
        <GlobalToasts />
      </div>
    </CouchProvider>
  );
}

export default App;
