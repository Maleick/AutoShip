import { Archive, CheckCircle, Warning, Spinner } from "@phosphor-icons/react";
import { useAdminBackups } from "../hooks/useAdminBackups";

function getStatusIcon(status: string) {
  switch (status) {
    case "success":
      return <CheckCircle className="h-5 w-5 text-emerald-300" />;
    case "failed":
      return <Warning className="h-5 w-5 text-rose-300" />;
    case "in_progress":
      return <Spinner className="h-5 w-5 animate-spin text-blue-300" />;
    default:
      return null;
  }
}

function getStatusColor(status: string) {
  switch (status) {
    case "success":
      return "border-emerald-400/30 bg-emerald-500/10 text-emerald-200";
    case "failed":
      return "border-rose-400/30 bg-rose-500/10 text-rose-200";
    case "in_progress":
      return "border-blue-400/30 bg-blue-500/10 text-blue-200";
    default:
      return "border-white/10 bg-white/5 text-white/70";
  }
}

function formatBytes(bytes: number) {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), sizes.length - 1);
  return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + " " + sizes[i];
}

export default function BackupBrowserPanel() {
  const { backups, loading, error } = useAdminBackups();

  if (error) {
    return (
      <div className="rounded-2xl border border-rose-400/30 bg-rose-500/10 p-6 text-rose-200">
        <div className="text-sm font-semibold">Backup Browser Unavailable</div>
        <div className="mt-2 text-xs text-rose-200/70">{error}</div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-6">
        <div className="animate-pulse text-white/50">Loading backups...</div>
      </div>
    );
  }

  if (backups.length === 0) {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-6">
        <div className="flex items-center gap-3">
          <Archive className="h-5 w-5 text-white/40" />
          <span className="text-sm text-white/40">No backups available</span>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="text-lg font-bold text-white">Backup Browser</div>

      <div className="max-h-96 space-y-2 overflow-y-auto rounded-2xl border border-white/10 bg-white/5 p-4">
        {backups.map((backup) => (
          <div
            key={backup.id}
            className={`rounded-lg border p-3 ${getStatusColor(backup.status)}`}
          >
            <div className="flex items-start justify-between gap-2">
              <div className="flex items-center gap-2">
                {getStatusIcon(backup.status)}
                <div>
                  <div className="font-semibold">{backup.id}</div>
                  <div className="text-xs opacity-70">
                    {new Date(backup.timestamp).toLocaleString()}
                  </div>
                </div>
              </div>
              <div className="text-right text-xs opacity-70">
                {formatBytes(backup.size_bytes)}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
