/** Server actions shared by the dashboard cards and the server page. */
import { api } from "../lib/api";
import { errorMessage, getState, setState } from "./store";

function setActionError(id: string, message: string | null) {
  setState((s) => {
    const next = { ...s.actionErrors };
    if (message) next[id] = message;
    else delete next[id];
    return { actionErrors: next };
  });
}

/** Runs a server action, recording a failure against the instance. */
export async function runAction(id: string, action: () => Promise<unknown>): Promise<boolean> {
  setActionError(id, null);
  try {
    await action();
    return true;
  } catch (e) {
    setActionError(id, errorMessage(e));
    return false;
  }
}

/**
 * Launches a server, first asking for the Minecraft EULA if it has never been
 * accepted (KTD15). Declining leaves everything untouched.
 */
export async function requestLaunch(id: string): Promise<void> {
  if (!getState().settings?.eulaAcceptedAt) {
    setState({ eulaPrompt: id });
    return;
  }
  await runAction(id, () => api.startServer(id));
}

export async function acceptEulaAndLaunch(): Promise<void> {
  const id = getState().eulaPrompt;
  try {
    const settings = await api.acceptEula();
    setState({ settings, eulaPrompt: null });
  } catch (e) {
    setState({ eulaPrompt: null, error: errorMessage(e) });
    return;
  }
  if (id) await runAction(id, () => api.startServer(id));
}

export function declineEula(): void {
  setState({ eulaPrompt: null });
}
