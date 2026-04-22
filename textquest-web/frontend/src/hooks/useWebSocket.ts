import { useEffect, useRef, useState } from "react";

type WsStatus = "connecting" | "open" | "closed" | "error";

interface UseWebSocketResult<T> {
  lastMessage: T | null;
  status: WsStatus;
  send: (msg: unknown) => void;
}

/**
 * WebSocket hook connecting to the textquest-web backend.
 * Proxy in vite.config.ts routes /ws → ws://localhost:3001
 */
export function useWebSocket<T>(path: string): UseWebSocketResult<T> {
  const [lastMessage, setLastMessage] = useState<T | null>(null);
  const [status, setStatus] = useState<WsStatus>("connecting");
  const ws = useRef<WebSocket | null>(null);

  useEffect(() => {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}${path}`;
    const socket = new WebSocket(url);
    ws.current = socket;

    socket.onopen = () => setStatus("open");
    socket.onclose = () => setStatus("closed");
    socket.onerror = () => setStatus("error");
    socket.onmessage = (event: MessageEvent) => {
      try {
        setLastMessage(JSON.parse(event.data as string) as T);
      } catch {
        // ignore malformed frames
      }
    };

    return () => {
      socket.close();
      ws.current = null;
    };
  }, [path]);

  const send = (msg: unknown) => {
    if (ws.current?.readyState === WebSocket.OPEN) {
      ws.current.send(JSON.stringify(msg));
    }
  };

  return { lastMessage, status, send };
}
