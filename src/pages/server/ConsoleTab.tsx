import { useEffect, useState } from "react";
import { ConsoleView } from "../../components/ConsoleView";
import { GlassButton } from "../../components/glass/GlassButton";
import { Icon } from "../../components/Icon";
import { api, type Instance, type Snapshot } from "../../lib/api";
import { loadConsole, useConsole } from "../../state/console";
import { errorMessage } from "../../state/store";

interface ConsoleTabProps {
  instance: Instance;
  snap: Snapshot;
}

export function ConsoleTab({ instance, snap }: ConsoleTabProps) {
  const lines = useConsole(instance.id);
  const [error, setError] = useState<string | null>(null);
  const running = snap.state === "starting" || snap.state === "online";

  useEffect(() => {
    loadConsole(instance.id);
  }, [instance.id]);

  const send = async (command: string) => {
    setError(null);
    try {
      await api.sendCommand(instance.id, command);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="console-layout">
      <div className="stack" style={{ minWidth: 0 }}>
        <ConsoleView lines={lines} onSend={send} inputDisabled={!running} />
        {error && <p className="error-text">{error}</p>}
      </div>
      <aside className="surface panel players-panel" aria-label="Players online">
        <h2 className="section-title">
          Online · {snap.players.length}/{instance.maxPlayers}
        </h2>
        {snap.players.length === 0 ? (
          <p className="muted">Nobody is online.</p>
        ) : (
          <ul className="player-list">
            {snap.players.map((p) => (
              <li key={p}>
                <span className="player-name">{p}</span>
                <span className="row" style={{ gap: 4 }}>
                  <GlassButton size="sm" onClick={() => send(`op ${p}`)} aria-label={`Make ${p} an operator`}>
                    Op
                  </GlassButton>
                  <GlassButton
                    size="sm"
                    variant="danger"
                    icon={<Icon name="kill" size={12} />}
                    onClick={() => send(`kick ${p}`)}
                    aria-label={`Kick ${p}`}
                  >
                    Kick
                  </GlassButton>
                </span>
              </li>
            ))}
          </ul>
        )}
      </aside>
    </div>
  );
}
