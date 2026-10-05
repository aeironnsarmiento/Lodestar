/** Server actions shared by the dashboard cards and the server page. */
import { api } from "../lib/api";
import { errorMessage, getState, refreshInstances, setState, type PendingStart } from "./store";

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

function perform(pending: PendingStart): Promise<boolean> {
  return pending.action === "launch"
    ? runAction(pending.id, () => api.startServer(pending.id))
    : runAction(pending.id, () => api.resetWorld(pending.id, pending.seed));
}

/**
 * Starts a server (launch or reset), first asking for the Minecraft EULA if it has
 * never been accepted (KTD15). Declining leaves everything untouched.
 */
async function requestStart(pending: PendingStart): Promise<void> {
  if (!getState().settings?.eulaAcceptedAt) {
    setState({ eulaPrompt: pending });
    return;
  }
  await perform(pending);
}

export function requestLaunch(id: string): Promise<void> {
  return requestStart({ id, action: "launch" });
}

/** Reset World: instant stop, fresh world (random seed unless one is given), start. */
export function requestReset(id: string, seed: string | null = null): Promise<void> {
  return requestStart({ id, action: "reset", seed: seed?.trim() || null });
}

/**
 * Reset World with new-world options. Turning hardcore on or off is saved to the
 * server first, so the new world is generated with it.
 */
export async function resetWorld(id: string, seed: string | null, hardcore: boolean): Promise<void> {
  const inst = getState().instances.find((i) => i.id === id);
  if (inst && inst.hardcore !== hardcore) {
    const saved = await runAction(id, async () => {
      await api.updateInstance({ ...inst, hardcore });
      await refreshInstances();
    });
    if (!saved) return;
  }
  await requestReset(id, seed);
}

export async function acceptEulaAndContinue(): Promise<void> {
  const pending = getState().eulaPrompt;
  try {
    const settings = await api.acceptEula();
    setState({ settings, eulaPrompt: null });
  } catch (e) {
    setState({ eulaPrompt: null, error: errorMessage(e) });
    return;
  }
  if (pending) await perform(pending);
}

export function declineEula(): void {
  setState({ eulaPrompt: null });
}
