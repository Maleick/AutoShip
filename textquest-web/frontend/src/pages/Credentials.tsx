import { type FormEvent, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Pencil,
  Play,
  Plus,
  Save,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";
import { Input } from "../components/Form.tsx";
import { ApiError, api } from "../lib/api.ts";

type AccountStatus = "active" | "locked" | "banned";

interface AccountRecord {
  id: string;
  name: string;
  server: string;
  character: string;
  class: string;
  group: number;
  status: AccountStatus;
  has_password: boolean;
}

interface AccountForm {
  name: string;
  server: string;
  character: string;
  class: string;
  group: string;
  status: AccountStatus;
  password: string;
  encryptedConfirmed: boolean;
}

interface CredentialTestResponse {
  ok: boolean;
  message: string;
}

interface LaunchStep {
  accountName: string;
  staggerSeconds: number;
}

type FormErrors = Partial<Record<keyof AccountForm, string>>;

interface SaveCredentialPayload {
  server: string;
  character: string;
  class: string;
  group: number;
  status: AccountStatus;
  password?: string;
}

interface CreateCredentialPayload extends SaveCredentialPayload {
  name: string;
  password: string;
}

const EMPTY_FORM: AccountForm = {
  name: "",
  server: "Bertoxxulous",
  character: "",
  class: "UNK",
  group: "0",
  status: "active",
  password: "",
  encryptedConfirmed: true,
};

const LAUNCH_SEQUENCE_KEY = "textquest.credentials.launchSequence";
const STATUS_OPTIONS: AccountStatus[] = ["active", "locked", "banned"];

function isMissingRoute(error: unknown) {
  return error instanceof ApiError && error.status === 404;
}

async function withCredentialFallback<T>(
  credentialsPath: string,
  accountsPath: string,
  run: (path: string) => Promise<T>,
) {
  try {
    return await run(credentialsPath);
  } catch (error) {
    if (credentialsPath !== accountsPath && isMissingRoute(error)) {
      return run(accountsPath);
    }
    throw error;
  }
}

const credentialsApi = {
  list: () =>
    withCredentialFallback<AccountRecord[]>("/credentials", "/accounts", (path) =>
      api.get<AccountRecord[]>(path),
    ),
  create: (payload: CreateCredentialPayload) =>
    withCredentialFallback<AccountRecord>("/credentials", "/accounts", (path) =>
      api.post<AccountRecord>(path, payload),
    ),
  update: (id: string, name: string, payload: SaveCredentialPayload) =>
    withCredentialFallback<AccountRecord>(
      `/credentials/${encodeURIComponent(id)}`,
      `/accounts/${encodeURIComponent(name)}`,
      (path) => api.put<AccountRecord>(path, payload),
    ),
  delete: (id: string, name: string) =>
    withCredentialFallback<void>(
      `/credentials/${encodeURIComponent(id)}`,
      `/accounts/${encodeURIComponent(name)}`,
      (path) => api.delete<void>(path),
    ),
  test: (id: string, name: string) =>
    withCredentialFallback<CredentialTestResponse>(
      `/credentials/${encodeURIComponent(id)}/test`,
      `/accounts/${encodeURIComponent(name)}/test`,
      (path) => api.post<CredentialTestResponse>(path, {}),
    ),
};

function formFromAccount(account: AccountRecord): AccountForm {
  return {
    name: account.name,
    server: account.server,
    character: account.character,
    class: account.class,
    group: String(account.group),
    status: account.status,
    password: "",
    encryptedConfirmed: true,
  };
}

function upsertAccount(accounts: AccountRecord[], account: AccountRecord) {
  return [...accounts.filter((item) => item.name !== account.name), account].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
}

function readLaunchSequence(): LaunchStep[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(LAUNCH_SEQUENCE_KEY);
    const parsed = raw ? (JSON.parse(raw) as LaunchStep[]) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function syncLaunchSequence(accounts: AccountRecord[], current: LaunchStep[]) {
  const accountNames = new Set(accounts.map((account) => account.name));
  const kept = current.filter((step) => accountNames.has(step.accountName));
  const known = new Set(kept.map((step) => step.accountName));
  const appended = accounts
    .filter((account) => !known.has(account.name))
    .map((account, index) => ({
      accountName: account.name,
      staggerSeconds: (kept.length + index) * 15,
    }));
  return [...kept, ...appended];
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : "Unexpected credentials error";
}

export function Credentials() {
  const [accounts, setAccounts] = useState<AccountRecord[]>([]);
  const [editingName, setEditingName] = useState<string | null>(null);
  const [form, setForm] = useState<AccountForm>(EMPTY_FORM);
  const [errors, setErrors] = useState<FormErrors>({});
  const [showPassword, setShowPassword] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testingName, setTestingName] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ type: "ok" | "error"; text: string } | null>(null);
  const [testResults, setTestResults] = useState<Record<string, CredentialTestResponse>>({});
  const [launchSequence, setLaunchSequence] = useState<LaunchStep[]>(() => readLaunchSequence());

  const accountByName = useMemo(
    () => new Map(accounts.map((account) => [account.name, account])),
    [accounts],
  );
  const editingAccount = editingName ? accountByName.get(editingName) : undefined;

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    credentialsApi
      .list()
      .then((records) => {
        if (cancelled) return;
        setAccounts(records);
        setLaunchSequence((current) => syncLaunchSequence(records, current));
      })
      .catch((error) => {
        if (cancelled) return;
        setNotice({ type: "error", text: errorMessage(error) });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(LAUNCH_SEQUENCE_KEY, JSON.stringify(launchSequence));
    }
  }, [launchSequence]);

  const setField = <K extends keyof AccountForm>(field: K, value: AccountForm[K]) => {
    setForm((current) => ({ ...current, [field]: value }));
    setErrors((current) => ({ ...current, [field]: undefined }));
  };

  const startNew = () => {
    setEditingName(null);
    setForm(EMPTY_FORM);
    setErrors({});
    setShowPassword(false);
  };

  const startEdit = (account: AccountRecord) => {
    setEditingName(account.name);
    setForm(formFromAccount(account));
    setErrors({});
    setShowPassword(false);
  };

  const validate = () => {
    const nextErrors: FormErrors = {};
    const group = Number(form.group);
    if (!form.name.trim()) nextErrors.name = "Account is required.";
    if (/\s/.test(form.name)) nextErrors.name = "Account cannot contain spaces.";
    if (!form.server.trim()) nextErrors.server = "Server is required.";
    if (!form.character.trim()) nextErrors.character = "Primary character is required.";
    if (!Number.isInteger(group) || group < 0) nextErrors.group = "Group must be zero or higher.";
    if (!editingAccount && !form.password) nextErrors.password = "Password is required.";
    if (!form.encryptedConfirmed) {
      nextErrors.encryptedConfirmed = "Confirm encrypted storage before saving.";
    }
    setErrors(nextErrors);
    return Object.keys(nextErrors).length === 0;
  };

  const saveAccount = async (event: FormEvent) => {
    event.preventDefault();
    if (!validate()) return;

    setSaving(true);
    setNotice(null);
    const payload = {
      server: form.server.trim(),
      character: form.character.trim(),
      class: form.class.trim().toUpperCase() || "UNK",
      group: Number(form.group),
      status: form.status,
      password: form.password || undefined,
    };

    try {
      const saved = editingAccount
        ? await credentialsApi.update(editingAccount.id, editingAccount.name, payload)
        : await credentialsApi.create({
            ...payload,
            name: form.name.trim(),
            password: form.password,
          });
      setAccounts((current) => upsertAccount(current, saved));
      setLaunchSequence((current) => syncLaunchSequence(upsertAccount(accounts, saved), current));
      setEditingName(saved.name);
      setForm(formFromAccount(saved));
      setNotice({ type: "ok", text: `${saved.name} saved.` });
    } catch (error) {
      setNotice({ type: "error", text: errorMessage(error) });
    } finally {
      setSaving(false);
    }
  };

  const deleteAccount = async (account: AccountRecord) => {
    if (!window.confirm(`Delete ${account.name}? This also removes its encrypted password.`)) {
      return;
    }

    setNotice(null);
    try {
      await credentialsApi.delete(account.id, account.name);
      const nextAccounts = accounts.filter((item) => item.name !== account.name);
      setAccounts(nextAccounts);
      setLaunchSequence((current) => syncLaunchSequence(nextAccounts, current));
      if (editingName === account.name) startNew();
      setNotice({ type: "ok", text: `${account.name} deleted.` });
    } catch (error) {
      setNotice({ type: "error", text: errorMessage(error) });
    }
  };

  const testCredential = async (account: AccountRecord) => {
    setTestingName(account.name);
    setNotice(null);
    try {
      const result = await credentialsApi.test(account.id, account.name);
      setTestResults((current) => ({ ...current, [account.name]: result }));
      setNotice({ type: result.ok ? "ok" : "error", text: result.message });
    } catch (error) {
      setNotice({ type: "error", text: errorMessage(error) });
    } finally {
      setTestingName(null);
    }
  };

  const moveLaunchStep = (index: number, direction: -1 | 1) => {
    setLaunchSequence((current) => {
      const nextIndex = index + direction;
      if (nextIndex < 0 || nextIndex >= current.length) return current;
      const next = [...current];
      [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
      return next;
    });
  };

  const setLaunchStagger = (accountName: string, value: string) => {
    const staggerSeconds = Math.max(0, Math.floor(Number(value) || 0));
    setLaunchSequence((current) =>
      current.map((step) =>
        step.accountName === accountName ? { ...step, staggerSeconds } : step,
      ),
    );
  };

  return (
    <div>
      <PageHeader
        title="Credentials"
        subtitle={
          <>
            <KeyRound className="w-3.5 h-3.5 text-neriak-magenta" strokeWidth={1.75} />
            <span>{accounts.length} accounts</span>
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

      <div className="grid gap-4 p-6 xl:grid-cols-[minmax(0,1fr)_320px]">
        <div className="space-y-4">
          {notice && (
            <section
              className={`flex items-start gap-2 rounded-md border px-3 py-2 font-mono text-xs ${
                notice.type === "ok"
                  ? "border-state-ok/40 bg-state-ok/5 text-state-ok"
                  : "border-state-danger/40 bg-state-danger/5 text-state-danger"
              }`}
            >
              {notice.type === "ok" ? (
                <CheckCircle2 className="mt-0.5 h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
              ) : (
                <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" strokeWidth={1.75} />
              )}
              <span>{notice.text}</span>
            </section>
          )}

          <section className="overflow-hidden rounded-md border border-neriak-dim bg-panel">
            <div className="flex items-center gap-2 border-b border-neriak-dim px-3 py-2 font-mono text-xs uppercase tracking-[0.15em] text-neriak-muted">
              <KeyRound className="h-3.5 w-3.5 text-neriak-magenta" strokeWidth={1.75} />
              accounts
              <button
                type="button"
                onClick={startNew}
                className="ml-auto flex items-center gap-1 text-neriak-magenta hover:text-neriak-magenta-bright"
              >
                <Plus className="h-3 w-3" strokeWidth={2} />
                add credential
              </button>
            </div>
            <table className="w-full font-mono text-sm">
              <thead className="bg-void text-[10px] uppercase tracking-[0.15em] text-neriak-muted">
                <tr>
                  <th className="px-3 py-2 text-left">account</th>
                  <th className="px-3 py-2 text-left">character</th>
                  <th className="px-3 py-2 text-left">server</th>
                  <th className="px-3 py-2 text-left">group</th>
                  <th className="px-3 py-2 text-left">credential</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {loading ? (
                  <tr>
                    <td className="px-3 py-3 text-neriak-muted" colSpan={6}>
                      Loading credentials…
                    </td>
                  </tr>
                ) : accounts.length === 0 ? (
                  <tr>
                    <td className="px-3 py-3 text-neriak-muted" colSpan={6}>
                      No credentials found. Add one to get started.
                    </td>
                  </tr>
                ) : (
                  accounts.map((account) => {
                    const testResult = testResults[account.name];
                    return (
                      <tr
                        key={account.id}
                        className="group border-t border-neriak-dim/40 hover:bg-elevated/40"
                      >
                        <td className="px-3 py-2 text-neriak-magenta">{account.name}</td>
                        <td className="px-3 py-2 text-neriak-text">
                          <div>{account.character}</div>
                          <div className="text-xs text-neriak-dim">{account.class}</div>
                        </td>
                        <td className="px-3 py-2 text-neriak-muted">{account.server}</td>
                        <td className="px-3 py-2 text-neriak-muted">{account.group}</td>
                        <td className="px-3 py-2">
                          <span
                            className={`inline-flex items-center gap-1 rounded border px-2 py-1 text-xs ${
                              account.has_password
                                ? "border-state-ok/40 text-state-ok"
                                : "border-state-warn/40 text-state-warn"
                            }`}
                          >
                            <ShieldCheck className="h-3 w-3" strokeWidth={1.75} />
                            {account.has_password ? "encrypted" : "missing"}
                          </span>
                          {testResult && (
                            <div
                              className={`mt-1 text-[11px] ${
                                testResult.ok ? "text-state-ok" : "text-state-warn"
                              }`}
                            >
                              {testResult.message}
                            </div>
                          )}
                        </td>
                        <td className="px-3 py-2">
                          <div className="flex justify-end gap-2">
                            <button
                              type="button"
                              onClick={() => testCredential(account)}
                              className="text-neriak-dim hover:text-state-ok disabled:opacity-50"
                              disabled={testingName === account.name}
                              aria-label={`Test ${account.name}`}
                            >
                              {testingName === account.name ? (
                                <Loader2 className="h-4 w-4 animate-spin" strokeWidth={1.75} />
                              ) : (
                                <Play className="h-4 w-4" strokeWidth={1.75} />
                              )}
                            </button>
                            <button
                              type="button"
                              onClick={() => startEdit(account)}
                              className="text-neriak-dim hover:text-neriak-magenta"
                              aria-label={`Edit ${account.name}`}
                            >
                              <Pencil className="h-4 w-4" strokeWidth={1.75} />
                            </button>
                            <button
                              type="button"
                              onClick={() => deleteAccount(account)}
                              className="text-neriak-dim hover:text-state-danger"
                              aria-label={`Delete ${account.name}`}
                            >
                              <Trash2 className="h-4 w-4" strokeWidth={1.75} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </section>

          <section className="rounded-md border border-neriak-dim bg-panel">
            <div className="flex items-center gap-2 border-b border-neriak-dim px-3 py-2 font-mono text-xs text-neriak-muted">
              <Play className="h-3.5 w-3.5 text-neriak-magenta" strokeWidth={1.75} />
              launch sequence
            </div>
            <div className="divide-y divide-neriak-dim/40">
              {launchSequence.length === 0 ? (
                <div className="px-3 py-4 font-mono text-sm text-neriak-muted">
                  Add an account to build a launch sequence.
                </div>
              ) : (
                launchSequence.map((step, index) => {
                  const account = accountByName.get(step.accountName);
                  return (
                    <div
                      key={step.accountName}
                      className="grid gap-3 px-3 py-3 font-mono text-sm md:grid-cols-[72px_minmax(0,1fr)_150px]"
                    >
                      <div className="flex items-center gap-1">
                        <button
                          type="button"
                          onClick={() => moveLaunchStep(index, -1)}
                          disabled={index === 0}
                          className="text-neriak-dim hover:text-neriak-magenta disabled:opacity-30"
                          aria-label={`Move ${step.accountName} earlier`}
                        >
                          <ArrowUp className="h-4 w-4" strokeWidth={1.75} />
                        </button>
                        <button
                          type="button"
                          onClick={() => moveLaunchStep(index, 1)}
                          disabled={index === launchSequence.length - 1}
                          className="text-neriak-dim hover:text-neriak-magenta disabled:opacity-30"
                          aria-label={`Move ${step.accountName} later`}
                        >
                          <ArrowDown className="h-4 w-4" strokeWidth={1.75} />
                        </button>
                      </div>
                      <div>
                        <div className="text-neriak-text">{step.accountName}</div>
                        <div className="text-xs text-neriak-dim">
                          {account ? `${account.server} · ${account.character}` : "account missing"}
                        </div>
                      </div>
                      <label className="grid gap-1 text-xs text-neriak-muted">
                        stagger (s)
                        <input
                          type="number"
                          min={0}
                          value={step.staggerSeconds}
                          onChange={(event) => setLaunchStagger(step.accountName, event.target.value)}
                          className="rounded border border-neriak-dim bg-void px-2 py-1 text-sm text-neriak-text"
                        />
                      </label>
                    </div>
                  );
                })
              )}
            </div>
          </section>
        </div>

        <aside className="space-y-4">
          <form
            onSubmit={saveAccount}
            className="rounded-md border border-neriak-dim bg-panel p-4 font-mono text-sm"
          >
            <div className="mb-4 flex items-center gap-2 text-xs text-neriak-muted">
              {editingAccount ? (
                <Pencil className="h-3.5 w-3.5 text-neriak-magenta" strokeWidth={1.75} />
              ) : (
                <Plus className="h-3.5 w-3.5 text-neriak-magenta" strokeWidth={1.75} />
              )}
              {editingAccount ? `edit ${editingAccount.name}` : "add account"}
            </div>

            <div className="grid gap-3">
              <Input
                label="Daybreak account"
                id="account-name"
                value={form.name}
                onChange={(event) => setField("name", event.target.value)}
                disabled={Boolean(editingAccount)}
                autoComplete="username"
                error={errors.name}
                className="disabled:text-neriak-dim"
              />

              <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-1">
                <Input
                  label="Server"
                  id="account-server"
                  value={form.server}
                  onChange={(event) => setField("server", event.target.value)}
                  error={errors.server}
                />

                <Input
                  label="Group"
                  id="account-group"
                  type="number"
                  min={0}
                  value={form.group}
                  onChange={(event) => setField("group", event.target.value)}
                  error={errors.group}
                />
              </div>

              <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-1">
                <Input
                  label="Primary character"
                  id="account-character"
                  value={form.character}
                  onChange={(event) => setField("character", event.target.value)}
                  error={errors.character}
                />

                <Input
                  label="Class"
                  id="account-class"
                  value={form.class}
                  onChange={(event) => setField("class", event.target.value.toUpperCase())}
                />
              </div>

              <label className="grid gap-1">
                <span className="text-xs text-neriak-muted">Status</span>
                <select
                  value={form.status}
                  onChange={(event) => setField("status", event.target.value as AccountStatus)}
                  className="rounded border border-neriak-dim bg-void px-3 py-2 text-neriak-text"
                >
                  {STATUS_OPTIONS.map((status) => (
                    <option key={status} value={status}>
                      {status}
                    </option>
                  ))}
                </select>
              </label>

              <label className="grid gap-1">
                <span className="text-xs text-neriak-muted">
                  Password {editingAccount ? "(leave blank to keep current)" : ""}
                </span>
                <span className="flex rounded border border-neriak-dim bg-void">
                  <input
                    type={showPassword ? "text" : "password"}
                    value={form.password}
                    onChange={(event) => setField("password", event.target.value)}
                    autoComplete={editingAccount ? "new-password" : "current-password"}
                    className="min-w-0 flex-1 bg-transparent px-3 py-2 text-neriak-text outline-none"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword((current) => !current)}
                    className="px-3 text-neriak-dim hover:text-neriak-magenta"
                    aria-label={showPassword ? "Hide password" : "Show password"}
                  >
                    {showPassword ? (
                      <EyeOff className="h-4 w-4" strokeWidth={1.75} />
                    ) : (
                      <Eye className="h-4 w-4" strokeWidth={1.75} />
                    )}
                  </button>
                </span>
                {errors.password && (
                  <span className="text-xs text-state-danger">{errors.password}</span>
                )}
              </label>

              <label className="flex items-start gap-2 rounded border border-state-ok/40 bg-state-ok/5 p-3 text-xs text-neriak-muted">
                <input
                  type="checkbox"
                  checked={form.encryptedConfirmed}
                  onChange={(event) => setField("encryptedConfirmed", event.target.checked)}
                  className="mt-0.5"
                />
                <span>
                  Store this password only through the encrypted backend credential store.
                  {errors.encryptedConfirmed && (
                    <span className="mt-1 block text-state-danger">
                      {errors.encryptedConfirmed}
                    </span>
                  )}
                </span>
              </label>

              <div className="flex flex-wrap gap-2 pt-2">
                <button
                  type="submit"
                  disabled={saving}
                  className="flex items-center gap-2 rounded border border-neriak-magenta/60 px-3 py-2 text-neriak-magenta hover:border-neriak-magenta hover:text-neriak-magenta-bright disabled:opacity-50"
                >
                  {saving ? (
                    <Loader2 className="h-4 w-4 animate-spin" strokeWidth={1.75} />
                  ) : (
                    <Save className="h-4 w-4" strokeWidth={1.75} />
                  )}
                  save
                </button>
                {editingAccount && (
                  <button
                    type="button"
                    onClick={startNew}
                    className="flex items-center gap-2 rounded border border-neriak-dim px-3 py-2 text-neriak-muted hover:text-neriak-text"
                  >
                    <X className="h-4 w-4" strokeWidth={1.75} />
                    cancel
                  </button>
                )}
              </div>
            </div>
          </form>

          <section className="rounded-md border border-state-warn/40 bg-state-warn/5 p-4">
            <div className="flex items-start gap-3">
              <ShieldCheck
                className="mt-0.5 h-4 w-4 shrink-0 text-state-warn"
                strokeWidth={1.75}
              />
              <div className="font-mono text-xs text-neriak-muted">
                <div className="mb-1 text-state-warn">encrypted storage</div>
                <p>
                  Saved passwords are sent to the backend credential store and returned to the UI
                  only as encrypted availability status.
                </p>
              </div>
            </div>
          </section>
        </aside>
      </div>
    </div>
  );
}
