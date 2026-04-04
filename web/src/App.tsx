import { useState } from "react";
import "./App.css";

type Tab = "credentials" | "groups" | "sessions" | "loot";

const TABS: { id: Tab; label: string }[] = [
  { id: "credentials", label: "Credentials" },
  { id: "groups", label: "Group Builder" },
  { id: "sessions", label: "Session Monitor" },
  { id: "loot", label: "Loot Tables" },
];

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("sessions");

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100">
      {/* Header */}
      <header className="border-b border-gray-800 px-6 py-4">
        <div className="flex items-center justify-between">
          <h1 className="text-xl font-bold tracking-tight">
            DMFT <span className="text-blue-400">Dashboard</span>
          </h1>
          <span className="text-xs text-gray-500">v0.1.0</span>
        </div>
      </header>

      {/* Tab Navigation */}
      <nav className="border-b border-gray-800 px-6">
        <div className="flex gap-1">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`px-4 py-3 text-sm font-medium transition-colors ${
                activeTab === tab.id
                  ? "border-b-2 border-blue-400 text-blue-400"
                  : "text-gray-400 hover:text-gray-200"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </nav>

      {/* Tab Content */}
      <main className="p-6">
        {activeTab === "credentials" && <CredentialsTab />}
        {activeTab === "groups" && <GroupBuilderTab />}
        {activeTab === "sessions" && <SessionMonitorTab />}
        {activeTab === "loot" && <LootTablesTab />}
      </main>
    </div>
  );
}

function CredentialsTab() {
  return (
    <div className="rounded-lg border border-gray-800 bg-gray-900 p-6">
      <h2 className="text-lg font-semibold mb-4">Credentials</h2>
      <p className="text-gray-400">
        Manage Daybreak account credentials. Encrypted with Argon2id +
        AES-256-GCM.
      </p>
    </div>
  );
}

function GroupBuilderTab() {
  return (
    <div className="rounded-lg border border-gray-800 bg-gray-900 p-6">
      <h2 className="text-lg font-semibold mb-4">Group Builder</h2>
      <p className="text-gray-400">
        Configure group compositions, camp assignments, and class strategies.
      </p>
    </div>
  );
}

function SessionMonitorTab() {
  return (
    <div className="rounded-lg border border-gray-800 bg-gray-900 p-6">
      <h2 className="text-lg font-semibold mb-4">Session Monitor</h2>
      <p className="text-gray-400">
        Real-time fleet overview. Connects via WebSocket for live updates.
      </p>
      <div className="mt-4 grid grid-cols-6 gap-3">
        {Array.from({ length: 6 }, (_, i) => (
          <div
            key={i}
            className="rounded border border-gray-700 bg-gray-800 p-3 text-center"
          >
            <div className="text-sm font-medium text-gray-300">
              Client {i + 1}
            </div>
            <div className="text-xs text-gray-500 mt-1">Idle</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function LootTablesTab() {
  return (
    <div className="rounded-lg border border-gray-800 bg-gray-900 p-6">
      <h2 className="text-lg font-semibold mb-4">Loot Tables</h2>
      <p className="text-gray-400">
        Item database, wishlists, and &ldquo;who needs this?&rdquo; tracking.
      </p>
    </div>
  );
}

export default App;
