import { useEffect, useRef, useCallback, useState } from "react";

export function useWebSocket(url: string) {
  const wsRef = useRef<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState<string | null>(null);

  useEffect(() => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let active = true;

    function connect() {
      const ws = new WebSocket(url);
      wsRef.current = ws;
      ws.onopen = () => {
        if (active) setConnected(true);
      };
      ws.onclose = () => {
        if (!active) return;
        setConnected(false);
        timeoutId = setTimeout(connect, 3000);
      };
      ws.onmessage = (e) =>
        setLastMessage(typeof e.data === "string" ? e.data : String(e.data));
      ws.onerror = () => ws.close();
    }

    connect();
    return () => {
      active = false;
      if (timeoutId !== null) clearTimeout(timeoutId);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [url]);

  const send = useCallback((data: string) => {
    wsRef.current?.send(data);
  }, []);

  return { connected, lastMessage, send };
}
