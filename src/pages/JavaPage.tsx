import { PageHeader } from "../components/PageHeader";

export function JavaPage() {
  return (
    <div className="page">
      <PageHeader title="Java runtimes" subtitle="Downloaded automatically for each server" />
      <div className="surface empty">No runtimes installed.</div>
    </div>
  );
}
