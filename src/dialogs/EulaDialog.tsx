import { openUrl } from "@tauri-apps/plugin-opener";
import { GlassButton } from "../components/glass/GlassButton";
import { Dialog } from "./Dialog";

export const EULA_URL = "https://aka.ms/MinecraftEULA";

interface EulaDialogProps {
  onAccept: () => void;
  onDecline: () => void;
}

/** Asked once, app-wide, before the first server launch (KTD15). */
export function EulaDialog({ onAccept, onDecline }: EulaDialogProps) {
  return (
    <Dialog
      title="Minecraft EULA"
      onClose={onDecline}
      footer={
        <>
          <GlassButton onClick={onDecline}>Decline</GlassButton>
          <GlassButton variant="primary" onClick={onAccept}>
            I agree
          </GlassButton>
        </>
      }
    >
      <p>
        Running a Minecraft server requires agreeing to the Minecraft End User License Agreement. Lodestar asks once
        and remembers your answer for every server.
      </p>
      <p>
        <a
          href={EULA_URL}
          onClick={(e) => {
            e.preventDefault();
            openUrl(EULA_URL).catch(() => {});
          }}
        >
          Read the Minecraft EULA
        </a>
      </p>
    </Dialog>
  );
}
