import { renderHook, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useWebSocket } from "./useWebSocket";

class MockWebSocket {
  static instances: MockWebSocket[] = [];

  url: string;
  onopen: null | (() => void) = null;
  onclose: null | (() => void) = null;
  onmessage: null | ((event: MessageEvent<string>) => void) = null;
  onerror: null | (() => void) = null;
  sent: string[] = [];
  closed = false;

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
    this.onclose?.();
  }

  triggerOpen() {
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
    expect(result.current.connected).toBe(true);
    expect(MockWebSocket.instances[0].sent).toEqual(["hello"]);

    await act(async () => {
      MockWebSocket.instances[0].triggerMessage("payload");
    });
    expect(result.current.lastMessage).toBe("payload");

    await act(async () => {
      MockWebSocket.instances[0].close();
    });
    expect(result.current.connected).toBe(false);

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
});
