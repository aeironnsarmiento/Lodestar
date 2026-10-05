import { PageHeader } from "../components/PageHeader";

export function Dashboard() {
  return (
    <div className="page">
      <PageHeader title="Dashboard" subtitle="Your Minecraft servers" />
      <div className="surface empty">No servers yet.</div>
    </div>
  );
}
