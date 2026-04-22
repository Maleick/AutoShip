import { Card } from "../components/Card.tsx";

export function Dashboard() {
  return (
    <div className="p-6 space-y-6">
      <h1 className="text-2xl font-bold text-gray-100">Dashboard</h1>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <Card title="Clients">
          <p className="text-3xl font-mono text-blue-400">0</p>
          <p className="text-sm text-gray-500 mt-1">Connected</p>
        </Card>
        <Card title="Groups">
          <p className="text-3xl font-mono text-green-400">0</p>
          <p className="text-sm text-gray-500 mt-1">Active</p>
        </Card>
        <Card title="Economy">
          <p className="text-3xl font-mono text-yellow-400">—</p>
          <p className="text-sm text-gray-500 mt-1">Plat balance</p>
        </Card>
      </div>
    </div>
  );
}
