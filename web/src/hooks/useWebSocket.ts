import { useEffect, useRef, useCallback, useState } from "react";

export interface UseWebSocketOptions<T = string> {
  /** Interval in milliseconds between reconnect attempts. Default: 3000 */
  reconnectInterval?: number;
  /** Maximum number of reconnect attempts. Default: Infinity */
  maxReconnectAttempts?: number;
  /** Called when WebSocket connection opens */
  onOpen?: () => void;
  /** Called when WebSocket connection closes */
  onClose?: () => void;
  /** Called when a WebSocket error occurs */
  onError?: (error: Event) => void;
  /** Called when a message is received */
  onMessage?: (data: T) => void;
}

export interface UseWebSocketReturn<T = string> {
  /** Whether the WebSocket is currently connected */
  isConnected: boolean;
  /** @deprecated Use isConnected instead. */
  connected: boolean;
  /** The last received message */
  lastMessage: T | null;
  /** Send a message over the WebSocket */
  send: (data: T | string) => void;
  /** Close the WebSocket connection */
  close: () => void;
}

/**
 * React hook for WebSocket connection management with auto-reconnect.
 *
 * @template T - The type of messages handled by this hook (default: string)
 * @param url - The WebSocket URL to connect to
 * @param options - Configuration options for the hook
 * @returns Object with connection state, send function, and close function
 *
 * @example
 * const { isConnected, lastMessage, send } = useWebSocket<MyMessageType>(
 *   'wss://example.com/ws',
 *   {
 *     reconnectInterval: 5000,
 *     maxReconnectAttempts: 5,
 *     onOpen: () => console.log('Connected'),
 *     onMessage: (data) => console.log('Message:', data),
 *   }
 * );
 */
export function useWebSocket<T = string>(
  url: string,
  options?: UseWebSocketOptions<T>
): UseWebSocketReturn<T> {
  const {
    reconnectInterval = 3000,
    maxReconnectAttempts = Infinity,
    onOpen,
    onClose,
    onError,
    onMessage,
  } = options || {};

  const wsRef = useRef<WebSocket | null>(null);
  const [isConnected, setIsConnected] = useState(false);
  const [lastMessage, setLastMessage] = useState<T | null>(null);
  const reconnectCountRef = useRef(0);
  const timeoutIdRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeRef = useRef(true);

  const closeWebSocket = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
  }, []);

  const connect = useCallback(() => {
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      if (!activeRef.current) return;
      setIsConnected(true);
      onOpen?.();
    };

    ws.onclose = () => {
      if (!activeRef.current) return;
      setIsConnected(false);
      onClose?.();

      if (reconnectCountRef.current < maxReconnectAttempts) {
        reconnectCountRef.current++;
        timeoutIdRef.current = setTimeout(connect, reconnectInterval);
      }
    };

    ws.onmessage = (e) => {
      if (!activeRef.current) return;

      // Keep lastMessage as the raw payload for backward compatibility
      // with existing consumers that parse string payloads themselves.
      setLastMessage(e.data as unknown as T);

      if (!onMessage) {
        return;
      }

      try {
        const callbackData: T =
          typeof e.data === "string" ? (JSON.parse(e.data) as T) : (e.data as T);
        onMessage(callbackData);
      } catch {
        onMessage(e.data as unknown as T);
      }
    };

    ws.onerror = (error) => {
      if (!activeRef.current) return;
      onError?.(error);
      ws.close();
    };
  }, [url, reconnectInterval, maxReconnectAttempts, onOpen, onClose, onError, onMessage]);

  useEffect(() => {
    activeRef.current = true;
    connect();

    return () => {
      activeRef.current = false;
      if (timeoutIdRef.current !== null) {
        clearTimeout(timeoutIdRef.current);
      }
      closeWebSocket();
    };
  }, [connect, closeWebSocket]);

  const send = useCallback(
    (data: T | string) => {
      if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
        const payload = typeof data === "string" ? data : JSON.stringify(data);
        wsRef.current.send(payload);
      }
    },
    []
  );

  return {
    isConnected,
    connected: isConnected,
    lastMessage,
    send,
    close: closeWebSocket,
  };
}
