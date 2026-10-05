import { PageHeader } from "../components/PageHeader";

export function Dashboard(_props: { onOpen: (id: string) => void }) {
  return (
    <div className="page">
      <PageHeader title="Dashboard" subtitle="Your Minecraft servers" />
      <div className="surface empty">No servers yet.</div>
    </div>
  );
}
