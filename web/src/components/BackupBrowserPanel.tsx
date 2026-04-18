import { Archive, Download, Plus, Trash } from "@phosphor-icons/react";
import { useState } from "react";
import { useAdminBackups } from "../hooks/useAdminBackups";

export function BackupBrowserPanel() {
  const { backups, loading, error } = useAdminBackups();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const handleCreateBackup = async () => {
    try {
      const response = await fetch("/api/admin/backups", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });
      if (response.ok) {
        // Trigger refresh by refetching
        window.location.reload();
      }
    } catch (e) {
      console.error("Failed to create backup:", e);
    }
  };

  const handleRestore = async (backupId: string) => {
    if (!confirm("Restore this backup? This action may take time.")) {
      return;
    }
    try {
      const response = await fetch(`/api/admin/backups/${backupId}/restore`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });
      if (response.ok) {
        alert("Restore initiated. Check logs for progress.");
      }
    } catch (e) {
      console.error("Failed to restore backup:", e);
    }
  };

  const handleDelete = async (backupId: string) => {
    if (!confirm("Delete this backup permanently?")) {
      return;
    }
    try {
      const response = await fetch(`/api/admin/backups/${backupId}`, {
        method: "DELETE",
      });
      if (response.ok) {
        window.location.reload();
      }
    } catch (e) {
      console.error("Failed to delete backup:", e);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + " " + sizes[i];
  };

  if (error) {
    return (
      <section className="rounded-[1.5rem] border border-rose-400/30 bg-rose-500/10 p-5">
        <div className="mb-3 flex items-center gap-3 text-rose-200">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-rose-400/30 bg-rose-500/10">
            <Archive className="text-rose-300" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Backups</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-rose-300/70">
              Error loading backups
            </p>
          </div>
        </div>
        <p className="text-sm text-rose-200/70">{error}</p>
      </section>
    );
  }

  if (loading) {
    return (
      <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
        <div className="mb-3 flex items-center gap-3 text-white">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
            <Archive className="text-amber-200 animate-pulse" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Backups</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
              Loading backups...
            </p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-3 text-white">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
            <Archive className="text-amber-200" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Backups</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
              {backups.length} available
            </p>
          </div>
        </div>
        <button
          onClick={handleCreateBackup}
          className="flex items-center gap-2 rounded-lg border border-amber-300/30 bg-amber-500/10 px-3 py-2 font-tech text-xs uppercase tracking-[0.18em] text-amber-200 transition hover:border-amber-300/60 hover:bg-amber-500/20"
        >
          <Plus size={16} />
          Create
        </button>
      </div>

      {backups.length === 0 ? (
        <p className="text-sm leading-6 text-white/60">
          No backups found. Create one using the button above to start archiving system state.
        </p>
      ) : (
        <div className="space-y-2">
          {backups.map((backup) => (
            <div
              key={backup.id}
              onClick={() => setSelectedId(selectedId === backup.id ? null : backup.id)}
              className="cursor-pointer rounded-lg border border-white/10 bg-white/5 p-3 transition hover:border-white/20 hover:bg-white/10"
            >
              <div className="flex items-center justify-between">
                <div className="flex flex-1 items-center gap-3">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 text-white">
                      <span className="text-sm font-semibold">
                        {new Date(backup.created_at).toLocaleString()}
                      </span>
                      <span
                        className={`text-xs px-2 py-1 rounded ${
                          backup.status === "completed"
                            ? "border border-emerald-300/30 bg-emerald-500/10 text-emerald-200"
                            : backup.status === "pending"
                              ? "border border-blue-300/30 bg-blue-500/10 text-blue-200"
                              : "border border-rose-300/30 bg-rose-500/10 text-rose-200"
                        }`}
                      >
                        {backup.status}
                      </span>
                    </div>
                    <p className="text-xs text-white/50">
                      Size: {formatBytes(backup.size_bytes)}
                    </p>
                    {backup.description && (
                      <p className="mt-1 text-xs text-white/40">{backup.description}</p>
                    )}
                  </div>
                </div>

                {backup.status === "completed" && (
                  <div className="flex gap-2">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRestore(backup.id);
                      }}
                      className="flex items-center gap-1 rounded px-2 py-1 text-xs text-blue-200 transition hover:bg-blue-500/20"
                      title="Restore this backup"
                    >
                      <Download size={14} />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(backup.id);
                      }}
                      className="flex items-center gap-1 rounded px-2 py-1 text-xs text-rose-200 transition hover:bg-rose-500/20"
                      title="Delete this backup"
                    >
                      <Trash size={14} />
                    </button>
                  </div>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

export default BackupBrowserPanel;
