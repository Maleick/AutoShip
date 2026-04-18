import { ShieldChevron } from "@phosphor-icons/react";

import { useAdminSessions } from "../hooks/useAdminSessions";
import { AdminSessionOverview } from "./AdminSessionOverview";
import { PerformancePanel } from "./PerformancePanel";
import { LogViewerPanel } from "./LogViewerPanel";
import { BackupBrowserPanel } from "./BackupBrowserPanel";

export function AdminDashboardPage() {
  const { sessions, loading, error } = useAdminSessions();

  return (
    <main className="min-h-screen bg-[radial-gradient(circle_at_top,#1a1b3a_0%,#09050f_58%,#050309_100%)] text-white">
      <div className="mx-auto flex min-h-screen max-w-7xl flex-col gap-8 px-6 py-8 lg:px-10">
        <header className="rounded-[2rem] border border-cyan-400/20 bg-[#120a1d]/88 px-6 py-6 shadow-[0_20px_60px_rgba(34,211,238,0.08)]">
          <div className="flex flex-col gap-5 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="font-tech text-xs uppercase tracking-[0.35em] text-cyan-200/80">
                Dedicated admin surface
              </p>
              <h1 className="mt-3 font-archaic text-4xl text-white">Admin Dashboard</h1>
              <p className="mt-3 max-w-3xl text-sm leading-6 text-white/65">
                Session inventory and operator-level navigation live here, separate from the
                primary dashboard. Follow-up issues add diagnostics, logs, and backups.
              </p>
            </div>
            <div className="flex flex-wrap gap-3">
              <a
                href="/"
                className="rounded-full border border-white/10 bg-white/5 px-4 py-2 font-tech text-xs uppercase tracking-[0.24em] text-white transition hover:border-cyan-300/40 hover:bg-white/10"
              >
                Return to dashboard
              </a>
            </div>
          </div>
        </header>

        <AdminSessionOverview
          sessions={sessions}
          loading={loading}
          error={error}
        />

        <div className="grid gap-4 lg:grid-cols-3">
          <PerformancePanel />
          <LogViewerPanel />
          <BackupBrowserPanel />
        </div>

        <footer className="pb-2 text-xs uppercase tracking-[0.24em] text-white/35">
          <span className="inline-flex items-center gap-2">
            <ShieldChevron className="text-cyan-200" size={16} />
            Admin route currently presents read-only session inventory.
          </span>
        </footer>
      </div>
    </main>
  );
}

export default AdminDashboardPage;
