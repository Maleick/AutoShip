import { useEffect, useRef, useCallback, useState } from "react";

export function useWebSocket(url: string) {
  const wsRef = useRef<WebSocket | null>(null);
  const [connected, setConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState<string | null>(null);

  useEffect(() => {
    let timeoutId: ReturnType<typeof setTimeout>;

    function connect() {
      const ws = new WebSocket(url);
      wsRef.current = ws;
      ws.onopen = () => setConnected(true);
      ws.onclose = () => {
        setConnected(false);
        timeoutId = setTimeout(connect, 3000);
      };
      ws.onmessage = (e) => setLastMessage(e.data);
      ws.onerror = () => ws.close();
    }

    connect();
    return () => {
      clearTimeout(timeoutId);
      wsRef.current?.close();
    };
  }, [url]);

  const send = useCallback((data: string) => {
    wsRef.current?.send(data);
  }, []);

  return { connected, lastMessage, send };
}
