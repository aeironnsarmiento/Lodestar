import { useEffect, useMemo, useState } from "react";
import { Sidebar, type AppArea } from "./components/Sidebar";
import { EffectsContext, RefractionFilter } from "./components/glass/effects";
import { Dashboard } from "./pages/Dashboard";
import { PlayitPage } from "./pages/PlayitPage";
import { JavaPage } from "./pages/JavaPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ServerPage } from "./pages/ServerPage";
import { inTauri } from "./lib/api";
import {
  applyReduceEffects,
  applyTheme,
  readStoredReduceEffects,
  readStoredTheme,
  supportsRefraction,
  type Theme,
} from "./lib/theme";
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
  const refraction = useMemo(supportsRefraction, []);
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
    <EffectsContext.Provider value={{ reduceEffects, refraction }}>
      <RefractionFilter />
      <div className="backdrop" aria-hidden="true" />
      <div className="app">
        <Sidebar active={area} onNavigate={navigate} theme={theme} onToggleTheme={toggleTheme} />
        <main className="main">
          {serverId && <ServerPage key={serverId} id={serverId} onBack={() => setServerId(null)} />}
          {!serverId && area === "dashboard" && <Dashboard onOpen={setServerId} />}
          {!serverId && area === "playit" && <PlayitPage />}
          {!serverId && area === "java" && <JavaPage />}
          {!serverId && area === "settings" && <SettingsPage reduceEffects={reduceEffects} onReduceEffects={changeReduceEffects} />}
        </main>
      </div>
      {eulaPrompt && <EulaDialog onAccept={acceptEulaAndContinue} onDecline={declineEula} />}
    </EffectsContext.Provider>
  );
}

export default App;
