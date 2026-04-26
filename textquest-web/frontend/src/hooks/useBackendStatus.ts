import { useEffect, useState } from "react";

export type BackendStatus = "checking" | "online" | "offline";

export function useBackendStatus(pollMs = 15000): BackendStatus {
  const [status, setStatus] = useState<BackendStatus>("checking");

  useEffect(() => {
    let cancelled = false;
    const check = async () => {
      try {
        const res = await fetch("/api/health", { credentials: "include" });
        if (!cancelled) setStatus(res.ok ? "online" : "offline");
      } catch (err) {
        // Network failure: keep status visible to operators via console so
        // intermittent connectivity issues are debuggable. The hook's caller
        // sees "offline" and can render its own UI.
        console.warn("[useBackendStatus] /api/health fetch failed:", err);
        if (!cancelled) setStatus("offline");
      }
    };
    check();
    const id = setInterval(check, pollMs);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [pollMs]);

  return status;
}
