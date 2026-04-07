import { useState, useEffect, useCallback } from "react";
import type {
  Account,
  CreateAccountPayload,
  UpdateAccountPayload,
} from "../types";

export function useAccounts() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAccounts = useCallback(async () => {
    try {
      const res = await fetch("/api/accounts");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: Account[] = await res.json();
      setAccounts(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch accounts");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAccounts();
  }, [fetchAccounts]);

  const createAccount = useCallback(
    async (payload: CreateAccountPayload): Promise<Account> => {
      const res = await fetch("/api/accounts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const created: Account = await res.json();
      setAccounts((prev) =>
        [...prev, created].sort((a, b) => a.name.localeCompare(b.name))
      );
      return created;
    },
    []
  );

  const updateAccount = useCallback(
    async (name: string, payload: UpdateAccountPayload): Promise<Account> => {
      const res = await fetch(`/api/accounts/${encodeURIComponent(name)}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const updated: Account = await res.json();
      setAccounts((prev) =>
        prev.map((a) => (a.name === name ? updated : a))
      );
      return updated;
    },
    []
  );

  const deleteAccount = useCallback(async (name: string): Promise<void> => {
    const res = await fetch(`/api/accounts/${encodeURIComponent(name)}`, {
      method: "DELETE",
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error ?? `HTTP ${res.status}`);
    }
    setAccounts((prev) => prev.filter((a) => a.name !== name));
  }, []);

  const setPassword = useCallback(
    async (name: string, password: string): Promise<void> => {
      const res = await fetch(
        `/api/accounts/${encodeURIComponent(name)}/password`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ password }),
        }
      );
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      setAccounts((prev) =>
        prev.map((a) => (a.name === name ? { ...a, has_password: true } : a))
      );
    },
    []
  );

  const exportAccounts = useCallback(async (): Promise<void> => {
    const res = await fetch("/api/accounts/export");
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    const blob = new Blob([JSON.stringify(data, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "textquest-accounts.json";
    a.click();
    URL.revokeObjectURL(url);
  }, []);

  const importAccounts = useCallback(
    async (file: File): Promise<number> => {
      const text = await file.text();
      let parsed: unknown;
      try {
        parsed = JSON.parse(text);
      } catch {
        throw new Error("Invalid JSON file");
      }
      // Accept either a bare array or { accounts: [...] }
      let accts: unknown[];
      if (Array.isArray(parsed)) {
        accts = parsed;
      } else if (
        parsed !== null &&
        typeof parsed === "object" &&
        "accounts" in parsed &&
        Array.isArray((parsed as Record<string, unknown>).accounts)
      ) {
        accts = (parsed as { accounts: unknown[] }).accounts;
      } else {
        throw new Error("Expected a JSON array or an object with an 'accounts' array");
      }

      const res = await fetch("/api/accounts/import", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ accounts: accts }),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      const result = await res.json();
      await fetchAccounts();
      return result.imported as number;
    },
    [fetchAccounts]
  );

  return {
    accounts,
    loading,
    error,
    refresh: fetchAccounts,
    createAccount,
    updateAccount,
    deleteAccount,
    setPassword,
    exportAccounts,
    importAccounts,
  };
}
