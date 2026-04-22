import { Gauge, Terminal, FolderSimple, Users, Warning, CheckCircle } from "@phosphor-icons/react";
import { useAdminSessions } from "../hooks/useAdminSessions";
import type { AdminSessionRecord } from "../types";

function SessionOverview({ sessions }: { sessions: AdminSessionRecord[] }) {
  if (sessions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-white/50">
        <Users size={48} className="mb-4" />
        <p>No sessions found</p>
        <p className="text-sm">Sessions will appear here when the orchestrator is running</p>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      {sessions.map((session) => (
        <div
          key={session.sessionId}
          className="flex items-center justify-between rounded-lg border border-white/10 bg-white/5 p-4"
        >
          <div className="flex items-center gap-4">
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-magentadark/20">
              <span className="font-mono text-sm text-magentaglow">
                {session.sessionId}
              </span>
            </div>
            <div>
              <p className="font-medium text-white">
                {session.characterName || "Unknown"}
              </p>
              <p className="text-sm text-white/50">
                {session.className || "Unknown"} • Group {session.groupId}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {session.lifecycle === "active" ? (
              <CheckCircle size={20} className="text-green-400" />
            ) : session.lifecycle === "error" ? (
              <Warning size={20} className="text-red-400" />
            ) : (
              <div className="h-2.5 w-2.5 rounded-full bg-yellow-400" />
            )}
            <span className="text-sm capitalize text-white/70">
              {session.lifecycle}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

function PerformancePanel() {
  return (
    <div className="rounded-lg border border-white/10 bg-white/5 p-4">
      <div className="mb-4 flex items-center gap-2">
        <Gauge size={20} className="text-magentaglow" />
        <h3 className="font-medium text-white">Performance</h3>
      </div>
      <div className="flex flex-col gap-4">
        <div className="text-center text-white/50">
          <p>Connect a session to view performance metrics</p>
        </div>
      </div>
    </div>
  );
}

function LogViewerPanel() {
  return (
    <div className="rounded-lg border border-white/10 bg-white/5 p-4">
      <div className="mb-4 flex items-center gap-2">
        <Terminal size={20} className="text-magentaglow" />
        <h3 className="font-medium text-white">Log Viewer</h3>
      </div>
      <div className="font-mono text-sm text-white/50">
        <p>Select a session to view logs</p>
      </div>
    </div>
  );
}

function BackupPanel() {
  return (
    <div className="rounded-lg border border-white/10 bg-white/5 p-4">
      <div className="mb-4 flex items-center gap-2">
        <FolderSimple size={20} className="text-magentaglow" />
        <h3 className="font-medium text-white">Backup Browser</h3>
      </div>
      <div className="text-center text-white/50">
        <p>No backups available</p>
      </div>
    </div>
  );
}

export default function AdminDashboard() {
  const { sessions, loading, error } = useAdminSessions();

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-white/50">Loading admin dashboard...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="text-red-400">Error: {error}</div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <h1 className="font-archaic text-2xl font-bold text-white">Admin Dashboard</h1>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <div className="space-y-6">
          <div className="rounded-lg border border-white/10 bg-white/5 p-4">
            <div className="mb-4 flex items-center gap-2">
              <Users size={20} className="text-magentaglow" />
              <h3 className="font-medium text-white">Sessions</h3>
            </div>
            <SessionOverview sessions={sessions} />
          </div>
        </div>

        <div className="space-y-6">
          <PerformancePanel />
          <LogViewerPanel />
          <BackupPanel />
        </div>
      </div>
    </div>
  );
}
