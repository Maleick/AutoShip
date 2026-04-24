import { useState } from "react";
import { Eye, EyeOff, KeyRound, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";

interface Credential {
  id: string;
  label: string;
  account: string;
  password_hint: string;
  server: string;
  last_used?: string;
}

const MOCK_CREDS: Credential[] = [
  {
    id: "c1",
    label: "Main Box 1",
    account: "maleick_01",
    password_hint: "••••••••",
    server: "Bertoxxulous",
    last_used: "today 03:12",
  },
  {
    id: "c2",
    label: "Main Box 2",
    account: "maleick_02",
    password_hint: "••••••••",
    server: "Bertoxxulous",
    last_used: "today 03:12",
  },
  {
    id: "c3",
    label: "CH Cleric",
    account: "maleick_clr",
    password_hint: "••••••••",
    server: "Bertoxxulous",
    last_used: "today 03:12",
  },
  {
    id: "c4",
    label: "Ench",
    account: "maleick_enc",
    password_hint: "••••••••",
    server: "Bertoxxulous",
    last_used: "today 03:12",
  },
  {
    id: "c5",
    label: "Shaman",
    account: "maleick_shm",
    password_hint: "••••••••",
    server: "Bertoxxulous",
    last_used: "today 03:12",
  },
  {
    id: "c6",
    label: "Necro",
    account: "maleick_nec",
    password_hint: "••••••••",
    server: "Bertoxxulous",
    last_used: "yesterday",
  },
];

export function Credentials() {
  const [creds, setCreds] = useState(MOCK_CREDS);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());

  const toggleReveal = (id: string) =>
    setRevealed((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const remove = (id: string) => setCreds((cs) => cs.filter((c) => c.id !== id));

  return (
    <div>
      <PageHeader
        title="Credentials"
        subtitle={
          <>
            <KeyRound className="w-3.5 h-3.5 text-neriak-magenta" strokeWidth={1.75} />
            <span>{creds.length} accounts</span>
            <span className="text-neriak-dim">·</span>
            <span className="flex items-center gap-1 text-state-ok">
              <ShieldCheck className="w-3 h-3" strokeWidth={1.75} />
              encrypted at rest
            </span>
          </>
        }
        meta={
          <span className="text-[10px] font-mono text-state-warn border border-state-warn/40 bg-state-warn/5 rounded-sm px-2 py-1 uppercase tracking-[0.2em]">
            mock data
          </span>
        }
      />

      <div className="p-6 space-y-4">
        <section className="border border-neriak-dim rounded-md bg-panel overflow-hidden">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-neriak-dim text-xs font-mono text-neriak-muted uppercase tracking-[0.15em]">
            <KeyRound className="w-3.5 h-3.5 text-neriak-magenta" strokeWidth={1.75} />
            accounts
            <button
              type="button"
              aria-label="Add credential"
              className="ml-auto flex items-center gap-1 text-neriak-magenta hover:text-neriak-magenta-bright"
            >
              <Plus className="w-3 h-3" strokeWidth={2} />
              add credential
            </button>
          </div>
          <table className="w-full font-mono text-sm">
            <thead className="text-neriak-muted uppercase tracking-[0.15em] text-[10px] bg-void">
              <tr>
                <th className="px-3 py-2 text-left">label</th>
                <th className="px-3 py-2 text-left">account</th>
                <th className="px-3 py-2 text-left">password</th>
                <th className="px-3 py-2 text-left">server</th>
                <th className="px-3 py-2 text-left">last used</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {creds.map((c) => {
                const isRevealed = revealed.has(c.id);
                return (
                  <tr
                    key={c.id}
                    className="group border-t border-neriak-dim/40 hover:bg-elevated/40"
                  >
                    <td className="px-3 py-2 text-neriak-text">{c.label}</td>
                    <td className="px-3 py-2 text-neriak-magenta">{c.account}</td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <span className="text-neriak-muted font-mono">
                          {isRevealed ? "redacted_in_demo" : c.password_hint}
                        </span>
                        <button
                          onClick={() => toggleReveal(c.id)}
                          className="text-neriak-dim hover:text-neriak-magenta"
                        >
                          {isRevealed ? (
                            <EyeOff className="w-3.5 h-3.5" strokeWidth={1.75} />
                          ) : (
                            <Eye className="w-3.5 h-3.5" strokeWidth={1.75} />
                          )}
                        </button>
                      </div>
                    </td>
                    <td className="px-3 py-2 text-neriak-muted">{c.server}</td>
                    <td className="px-3 py-2 text-neriak-dim text-xs">{c.last_used ?? "—"}</td>
                    <td className="px-3 py-2 text-right">
                      <button
                        onClick={() => remove(c.id)}
                        className="opacity-0 group-hover:opacity-100 text-neriak-dim hover:text-state-danger transition-opacity"
                      >
                        <Trash2 className="w-3.5 h-3.5" strokeWidth={1.75} />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </section>

        <section className="border border-state-warn/40 bg-state-warn/5 rounded-md p-4 flex items-start gap-3">
          <ShieldCheck className="w-4 h-4 text-state-warn shrink-0 mt-0.5" strokeWidth={1.75} />
          <div className="font-mono text-xs text-neriak-muted space-y-1">
            <div className="text-state-warn uppercase tracking-[0.2em] text-[10px]">
              security notice
            </div>
            <p>
              Credentials stored locally in the textquest-web backend, encrypted with the operator
              key. Passwords never leave the host and are decrypted only in-memory at client launch.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}
