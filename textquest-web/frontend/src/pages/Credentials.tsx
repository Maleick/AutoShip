import { Card } from "../components/Card.tsx";

export function Credentials() {
  return (
    <div className="p-6 space-y-6">
      <h1 className="text-2xl font-bold text-gray-100">Credentials</h1>
      <Card title="Account Credentials">
        <p className="text-gray-400 text-sm">No credentials stored yet.</p>
      </Card>
    </div>
  );
}
