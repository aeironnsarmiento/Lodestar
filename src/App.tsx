import { useMemo, useState } from "react";
import { Sidebar, type AppArea } from "./components/Sidebar";
import { EffectsContext, RefractionFilter } from "./components/glass/effects";
import { Dashboard } from "./pages/Dashboard";
import { PlayitPage } from "./pages/PlayitPage";
import { JavaPage } from "./pages/JavaPage";
import { SettingsPage } from "./pages/SettingsPage";
import {
  applyReduceEffects,
  applyTheme,
  readStoredReduceEffects,
  readStoredTheme,
  supportsRefraction,
  type Theme,
} from "./lib/theme";

function App() {
  const [area, setArea] = useState<AppArea>("dashboard");
  const [theme, setTheme] = useState<Theme>(() => {
    const t = readStoredTheme();
    applyTheme(t);
    return t;
  });
  const [reduceEffects, setReduceEffects] = useState(readStoredReduceEffects);
  const refraction = useMemo(supportsRefraction, []);

  const toggleTheme = () => {
    const next: Theme = theme === "dark" ? "light" : "dark";
    applyTheme(next);
    setTheme(next);
  };

  const changeReduceEffects = (reduce: boolean) => {
    applyReduceEffects(reduce);
    setReduceEffects(reduce);
  };

  return (
    <EffectsContext.Provider value={{ reduceEffects, refraction }}>
      <RefractionFilter />
      <div className="backdrop" aria-hidden="true" />
      <div className="app">
        <Sidebar active={area} onNavigate={setArea} theme={theme} onToggleTheme={toggleTheme} />
        <main className="main">
          {area === "dashboard" && <Dashboard />}
          {area === "playit" && <PlayitPage />}
          {area === "java" && <JavaPage />}
          {area === "settings" && <SettingsPage reduceEffects={reduceEffects} onReduceEffects={changeReduceEffects} />}
        </main>
      </div>
    </EffectsContext.Provider>
  );
}

export default App;
