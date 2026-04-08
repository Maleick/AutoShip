import { useState, useRef } from "react";
import {
  Plus,
  PencilSimple,
  Trash,
  DownloadSimple,
  UploadSimple,
  Key,
  LockSimple,
  LockSimpleOpen,
  Warning,
  CheckCircle,
  ArrowClockwise,
} from "@phosphor-icons/react";
import { useAccounts } from "../hooks/useAccounts";
import AccountModal from "./AccountModal";
import type { Account, AccountStatus, CreateAccountPayload, UpdateAccountPayload } from "../types";

// ─── Status badge ─────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: AccountStatus }) {
  if (status === "active") {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] uppercase tracking-widest border border-green-500/40 bg-green-900/20 text-green-400 font-rune">
        <CheckCircle size={10} weight="fill" /> Active
      </span>
    );
  }
  if (status === "locked") {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] uppercase tracking-widest border border-yellow-500/40 bg-yellow-900/20 text-yellow-400 font-rune">
        <LockSimple size={10} weight="fill" /> Locked
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] uppercase tracking-widest border border-red-500/40 bg-red-900/20 text-red-400 font-rune">
      <Warning size={10} weight="fill" /> Banned
    </span>
  );
}

// ─── Delete confirmation dialog ───────────────────────────────────────────────

function DeleteConfirm({
  name,
  onConfirm,
  onCancel,
}: {
  name: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-void border border-red-900/50 p-6 w-full max-w-sm shadow-[0_0_30px_rgba(239,68,68,0.15)]">
        <h3 className="font-archaic text-lg text-red-400 mb-2">Expunge Account?</h3>
        <p className="text-white/70 text-sm mb-6">
          Permanently remove{" "}
          <span className="text-white font-bold">{name}</span> from the registry.
          This cannot be undone.
        </p>
        <div className="flex gap-3">
          <button
            onClick={onCancel}
            className="flex-1 px-4 py-2 border border-white/20 text-white/60 text-sm hover:bg-white/5 transition-colors font-tech uppercase tracking-wider"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className="flex-1 px-4 py-2 bg-red-900/30 border border-red-500 text-red-300 text-sm hover:bg-red-900/50 transition-colors font-tech uppercase tracking-wider"
          >
            Expunge
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Password badge ───────────────────────────────────────────────────────────

function PasswordBadge({ hasPassword }: { hasPassword: boolean }) {
  return hasPassword ? (
    <span className="inline-flex items-center gap-1 text-spectral/80 font-rune text-[10px]">
      <LockSimpleOpen size={12} /> Stored
    </span>
  ) : (
    <span className="inline-flex items-center gap-1 text-white/30 font-rune text-[10px]">
      <Key size={12} /> None
    </span>
  );
}

// ─── Main AccountsPage ────────────────────────────────────────────────────────

export default function AccountsPage() {
  const {
    accounts,
    loading,
    error,
    refresh,
    createAccount,
    updateAccount,
    deleteAccount,
    exportAccounts,
    importAccounts,
  } = useAccounts();

  const [modalAccount, setModalAccount] = useState<Account | null | undefined>(
    undefined // undefined = closed, null = create mode, Account = edit mode
  );
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [toast, setToast] = useState<{ msg: string; kind: "ok" | "err" } | null>(null);
  const [search, setSearch] = useState("");
  const importRef = useRef<HTMLInputElement>(null);

  const showToast = (msg: string, kind: "ok" | "err") => {
    setToast({ msg, kind });
    setTimeout(() => setToast(null), 3500);
  };

  const handleSave = async (
    payload: CreateAccountPayload | UpdateAccountPayload,
    isNew: boolean
  ) => {
    if (isNew) {
      await createAccount(payload as CreateAccountPayload);
      showToast("Account enlisted successfully", "ok");
    } else {
      const name = modalAccount!.name;
      await updateAccount(name, payload as UpdateAccountPayload);
      showToast("Account updated", "ok");
    }
  };

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteAccount(deleteTarget);
      showToast(`Account '${deleteTarget}' expunged`, "ok");
    } catch (e) {
      showToast(e instanceof Error ? e.message : "Delete failed", "err");
    } finally {
      setDeleteTarget(null);
    }
  };

  const handleImport = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const count = await importAccounts(file);
      showToast(`Imported ${count} account(s)`, "ok");
    } catch (e) {
      showToast(e instanceof Error ? e.message : "Import failed", "err");
    } finally {
      e.target.value = "";
    }
  };

  const filtered = accounts.filter(
    (a) =>
      a.name.toLowerCase().includes(search.toLowerCase()) ||
      a.character.toLowerCase().includes(search.toLowerCase()) ||
      a.server.toLowerCase().includes(search.toLowerCase())
  );

  const statusCounts = {
    active: accounts.filter((a) => a.status === "active").length,
    locked: accounts.filter((a) => a.status === "locked").length,
    banned: accounts.filter((a) => a.status === "banned").length,
  };

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Toast notification */}
      {toast && (
        <div
          className={`absolute top-4 right-4 z-50 px-4 py-2 text-sm font-rune border shadow-lg ${
            toast.kind === "ok"
              ? "bg-green-900/80 border-green-500/50 text-green-300"
              : "bg-red-900/80 border-red-500/50 text-red-300"
          }`}
        >
          {toast.msg}
        </div>
      )}

      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Key weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Account Registry
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              {accounts.length} enlisted · {statusCounts.active} active
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={refresh}
            className="p-2 border border-white/10 text-white/40 hover:text-white hover:border-white/30 transition-colors"
            title="Refresh"
          >
            <ArrowClockwise size={16} />
          </button>
          <button
            onClick={() =>
              exportAccounts().catch((e) =>
                showToast(e instanceof Error ? e.message : "Export failed", "err")
              )
            }
            className="px-3 py-1.5 border border-spectral/30 text-spectral text-xs font-tech uppercase tracking-wider hover:bg-spectral/10 transition-colors flex items-center gap-1.5"
          >
            <DownloadSimple size={14} /> Export
          </button>
          <button
            onClick={() => importRef.current?.click()}
            className="px-3 py-1.5 border border-white/20 text-white/60 text-xs font-tech uppercase tracking-wider hover:bg-white/5 transition-colors flex items-center gap-1.5"
          >
            <UploadSimple size={14} /> Import
          </button>
          <input
            ref={importRef}
            type="file"
            accept=".json"
            onChange={handleImport}
            className="hidden"
          />
          <button
            onClick={() => setModalAccount(null)}
            className="px-4 py-1.5 bg-magentadark/20 border border-magentaglow text-white text-xs font-tech uppercase tracking-wider hover:bg-magentadark/40 transition-colors shadow-[0_0_12px_rgba(204,68,255,0.25)] flex items-center gap-1.5"
          >
            <Plus size={14} /> Enlist Account
          </button>
        </div>
      </header>

      {/* Status summary strip */}
      <div className="flex items-center gap-6 px-6 py-3 border-b border-white/5 bg-violet/10">
        <span className="text-xs font-rune text-green-400 flex items-center gap-1.5">
          <CheckCircle size={12} weight="fill" /> {statusCounts.active} Active
        </span>
        <span className="text-xs font-rune text-yellow-400 flex items-center gap-1.5">
          <LockSimple size={12} weight="fill" /> {statusCounts.locked} Locked
        </span>
        <span className="text-xs font-rune text-red-400 flex items-center gap-1.5">
          <Warning size={12} weight="fill" /> {statusCounts.banned} Banned
        </span>
        <div className="ml-auto">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Filter accounts…"
            className="bg-void border border-white/15 text-white text-xs px-3 py-1.5 w-52 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/25"
          />
        </div>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-y-auto">
        {loading && (
          <div className="flex items-center justify-center h-32 text-white/40 font-rune text-sm">
            Loading accounts…
          </div>
        )}
        {error && !loading && (
          <div className="flex items-center justify-center h-32 text-red-400 font-rune text-sm">
            {error}
          </div>
        )}
        {!loading && !error && filtered.length === 0 && (
          <div className="flex flex-col items-center justify-center h-48 gap-3 text-white/30">
            <Key size={32} weight="thin" />
            <p className="font-rune text-sm">
              {search ? "No accounts match the filter" : "No accounts enlisted yet"}
            </p>
            {!search && (
              <button
                onClick={() => setModalAccount(null)}
                className="mt-2 px-4 py-2 border border-magentadark/50 text-magentaglow text-xs font-tech uppercase tracking-wider hover:bg-magentadark/20 transition-colors"
              >
                + Enlist First Account
              </button>
            )}
          </div>
        )}
        {!loading && !error && filtered.length > 0 && (
          <table className="w-full text-sm">
            <thead className="sticky top-0 z-10">
              <tr className="bg-void border-b border-white/10 text-[10px] uppercase tracking-widest text-white/40 font-rune">
                <th className="text-left px-5 py-3">Account</th>
                <th className="text-left px-3 py-3">Character</th>
                <th className="text-left px-3 py-3">Server</th>
                <th className="text-left px-3 py-3">Class</th>
                <th className="text-left px-3 py-3">Grp</th>
                <th className="text-left px-3 py-3">Status</th>
                <th className="text-left px-3 py-3">Password</th>
                <th className="text-right px-5 py-3">Actions</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((account, i) => (
                <tr
                  key={account.name}
                  className={`border-b border-white/5 hover:bg-white/[0.03] transition-colors ${
                    i % 2 === 0 ? "" : "bg-white/[0.01]"
                  }`}
                >
                  <td className="px-5 py-3">
                    <span className="text-white font-medium font-tech">{account.name}</span>
                  </td>
                  <td className="px-3 py-3 text-white/80 font-rune">{account.character}</td>
                  <td className="px-3 py-3 text-white/60 font-rune text-xs">{account.server}</td>
                  <td className="px-3 py-3">
                    <span className="px-1.5 py-0.5 bg-violet/50 border border-white/10 text-spectral font-rune text-[11px]">
                      {account.class}
                    </span>
                  </td>
                  <td className="px-3 py-3 text-white/50 font-rune">
                    {account.group === 0 ? <span className="text-white/25">—</span> : account.group}
                  </td>
                  <td className="px-3 py-3">
                    <StatusBadge status={account.status} />
                  </td>
                  <td className="px-3 py-3">
                    <PasswordBadge hasPassword={account.has_password} />
                  </td>
                  <td className="px-5 py-3">
                    <div className="flex items-center justify-end gap-2">
                      <button
                        onClick={() => setModalAccount(account)}
                        className="p-1.5 text-white/40 hover:text-spectral transition-colors border border-transparent hover:border-spectral/30"
                        title="Edit account"
                      >
                        <PencilSimple size={14} />
                      </button>
                      <button
                        onClick={() => setDeleteTarget(account.name)}
                        className="p-1.5 text-white/40 hover:text-red-400 transition-colors border border-transparent hover:border-red-500/30"
                        title="Delete account"
                      >
                        <Trash size={14} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* Create / Edit modal */}
      {modalAccount !== undefined && (
        <AccountModal
          account={modalAccount}
          onClose={() => setModalAccount(undefined)}
          onSave={handleSave}
        />
      )}

      {/* Delete confirmation */}
      {deleteTarget && (
        <DeleteConfirm
          name={deleteTarget}
          onConfirm={handleDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </section>
  );
}
