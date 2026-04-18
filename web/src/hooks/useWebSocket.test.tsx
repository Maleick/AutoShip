import { renderHook, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useWebSocket } from "./useWebSocket";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static readonly OPEN = 1;
  static readonly CLOSED = 3;

  url: string;
  onopen: null | (() => void) = null;
  onclose: null | (() => void) = null;
  onmessage: null | ((event: MessageEvent<string>) => void) = null;
  onerror: null | (() => void) = null;
  sent: string[] = [];
  closed = false;
  readyState = MockWebSocket.OPEN;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  send(data: string) {
    this.sent.push(data);
  }

  close() {
    if (this.closed) return;
    this.closed = true;
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.();
  }

  triggerOpen() {
    this.readyState = MockWebSocket.OPEN;
    this.onopen?.();
  }

  triggerMessage(data: string) {
    this.onmessage?.({ data } as MessageEvent<string>);
  }

  triggerError() {
    this.onerror?.();
  }
}

describe("useWebSocket", () => {
  beforeEach(() => {
    MockWebSocket.instances = [];
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("tracks connection state, receives messages, and reconnects after close", async () => {
    const { result, unmount } = renderHook(() => useWebSocket("wss://example.test/ws"));
    expect(MockWebSocket.instances).toHaveLength(1);

    await act(async () => {
      result.current.send("hello");
      MockWebSocket.instances[0].triggerOpen();
    });
    expect(result.current.isConnected).toBe(true);
    expect(MockWebSocket.instances[0].sent).toEqual(["hello"]);

    await act(async () => {
      MockWebSocket.instances[0].triggerMessage("payload");
    });
    expect(result.current.lastMessage).toBe("payload");

    await act(async () => {
      MockWebSocket.instances[0].close();
    });
    expect(result.current.isConnected).toBe(false);

    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(MockWebSocket.instances).toHaveLength(2);

    unmount();
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    expect(MockWebSocket.instances).toHaveLength(2);
  });

  it("calls onOpen callback when connection is established", async () => {
    const onOpen = vi.fn();
    renderHook(() => useWebSocket("wss://example.test/ws", { onOpen }));

    await act(async () => {
      MockWebSocket.instances[0].triggerOpen();
    });
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("calls onClose callback when connection closes", async () => {
    const onClose = vi.fn();
    renderHook(() => useWebSocket("wss://example.test/ws", { onClose }));

    await act(async () => {
      MockWebSocket.instances[0].triggerOpen();
    });

    await act(async () => {
      MockWebSocket.instances[0].close();
    });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onError callback when error occurs", async () => {
    const onError = vi.fn();
    renderHook(() => useWebSocket("wss://example.test/ws", { onError }));

    await act(async () => {
      MockWebSocket.instances[0].triggerError();
    });
    expect(onError).toHaveBeenCalledTimes(1);
  });

  it("calls onMessage callback with typed messages", async () => {
    interface TestMessage {
      type: string;
      value: number;
    }

    const onMessage = vi.fn();
    renderHook(() => useWebSocket<TestMessage>("wss://example.test/ws", { onMessage }));

    await act(async () => {
      MockWebSocket.instances[0].triggerOpen();
    });

    await act(async () => {
      MockWebSocket.instances[0].triggerMessage('{"type":"test","value":42}');
    });

    expect(onMessage).toHaveBeenCalledWith({ type: "test", value: 42 });
  });

  it("respects maxReconnectAttempts option", async () => {
    renderHook(() =>
      useWebSocket("wss://example.test/ws", {
        maxReconnectAttempts: 2,
        reconnectInterval: 100,
      })
    );

    expect(MockWebSocket.instances).toHaveLength(1);

    await act(async () => {
      MockWebSocket.instances[0].triggerOpen();
    });

    await act(async () => {
      MockWebSocket.instances[0].close();
    });

    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    expect(MockWebSocket.instances).toHaveLength(2);

    await act(async () => {
      MockWebSocket.instances[1].triggerOpen();
    });

    await act(async () => {
      MockWebSocket.instances[1].close();
    });

    await act(async () => {
      vi.advanceTimersByTime(100);
    });

    expect(MockWebSocket.instances).toHaveLength(3);

    await act(async () => {
      MockWebSocket.instances[2].triggerOpen();
    });

    await act(async () => {
      MockWebSocket.instances[2].close();
    });

    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    // Should not create a fourth connection (already tried 2 reconnects)
    expect(MockWebSocket.instances).toHaveLength(3);
  });

  it("allows closing the connection via close() method", async () => {
    const { result } = renderHook(() => useWebSocket("wss://example.test/ws"));

    await act(async () => {
      MockWebSocket.instances[0].triggerOpen();
    });
    expect(result.current.isConnected).toBe(true);

    await act(async () => {
      result.current.close();
    });
    expect(MockWebSocket.instances[0].closed).toBe(true);
  });

  it("handles custom reconnect interval", async () => {
    renderHook(() => useWebSocket("wss://example.test/ws", { reconnectInterval: 5000 }));

    await act(async () => {
      MockWebSocket.instances[0].triggerOpen();
    });

    await act(async () => {
      MockWebSocket.instances[0].close();
    });

    await act(async () => {
      vi.advanceTimersByTime(4999);
    });
    expect(MockWebSocket.instances).toHaveLength(1);

    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(MockWebSocket.instances).toHaveLength(2);
  });
});
