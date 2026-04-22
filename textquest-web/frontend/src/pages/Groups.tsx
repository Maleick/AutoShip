import { Card } from "../components/Card.tsx";

export function Groups() {
  return (
    <div className="p-6 space-y-6">
      <h1 className="text-2xl font-bold text-gray-100">Groups</h1>
      <Card title="Active Groups">
        <p className="text-gray-400 text-sm">No groups configured yet.</p>
      </Card>
    </div>
  );
}
