import { useEffect, useState } from "react";
import { Sidebar, type AppArea } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { HeaderSlotContext } from "./components/PageHeader";
import { GlassPanel } from "./components/glass/GlassPanel";
import { EffectsContext } from "./components/glass/effects";
import { Dashboard } from "./pages/Dashboard";
import { PlayitPage } from "./pages/PlayitPage";
import { JavaPage } from "./pages/JavaPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ServerPage } from "./pages/ServerPage";
import { inTauri } from "./lib/api";
import { applyReduceEffects, applyTheme, readStoredReduceEffects, readStoredTheme, type Theme } from "./lib/theme";
import { loadSettings, saveSettings, useStore } from "./state/store";
import { acceptEulaAndContinue, declineEula } from "./state/actions";
import { EulaDialog } from "./dialogs/EulaDialog";
import { startSync } from "./state/sync";

function App() {
  const [area, setArea] = useState<AppArea>("dashboard");
  const [serverId, setServerId] = useState<string | null>(null);
  const [theme, setTheme] = useState<Theme>(() => {
    const t = readStoredTheme();
    applyTheme(t);
    return t;
  });
  const [reduceEffects, setReduceEffects] = useState(readStoredReduceEffects);
  const [headerSlot, setHeaderSlot] = useState<HTMLDivElement | null>(null);
  const eulaPrompt = useStore((s) => s.eulaPrompt);

  // The backend settings file is the source of truth for theme and effects.
  useEffect(() => {
    if (!inTauri()) return;
    loadSettings()
      .then((s) => {
        applyTheme(s.theme);
        setTheme(s.theme);
        applyReduceEffects(s.reduceEffects);
        setReduceEffects(s.reduceEffects);
      })
      .catch(() => {});
    return startSync();
  }, []);

  const navigate = (next: AppArea) => {
    setServerId(null);
    setArea(next);
  };

  const toggleTheme = () => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    applyTheme(next);
    setTheme(next);
    if (inTauri()) saveSettings({ theme: next });
  };

  const changeReduceEffects = (reduce: boolean) => {
    applyReduceEffects(reduce);
    setReduceEffects(reduce);
    if (inTauri()) saveSettings({ reduceEffects: reduce });
  };

  return (
    <EffectsContext.Provider value={{ reduceEffects }}>
      <HeaderSlotContext.Provider value={headerSlot}>
        <div className="wallpaper" aria-hidden="true" />
        <div className="viewport">
          <GlassPanel className="shell" radius="var(--radius-xl)">
            <TopBar slotRef={setHeaderSlot} theme={theme} onToggleTheme={toggleTheme} />
            <Sidebar active={area} onNavigate={navigate} />
            <main className="main">
              {serverId && <ServerPage key={serverId} id={serverId} onBack={() => setServerId(null)} onOpenSettings={() => navigate("settings")} />}
              {!serverId && area === "dashboard" && <Dashboard onOpen={setServerId} onOpenSettings={() => navigate("settings")} />}
              {!serverId && area === "playit" && <PlayitPage />}
              {!serverId && area === "java" && <JavaPage />}
              {!serverId && area === "settings" && (
                <SettingsPage reduceEffects={reduceEffects} onReduceEffects={changeReduceEffects} />
              )}
            </main>
          </GlassPanel>
        </div>
        {eulaPrompt && <EulaDialog onAccept={acceptEulaAndContinue} onDecline={declineEula} />}
      </HeaderSlotContext.Provider>
    </EffectsContext.Provider>
  );
}

export default App;
