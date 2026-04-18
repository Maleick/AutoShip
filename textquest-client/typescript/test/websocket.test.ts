import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { WebSocket, WebSocketServer, type RawData } from "ws";

import {
  type TextQuestWebSocketLike,
  TextQuestWebSocketError,
  createTextQuestWebSocketUrl,
  subscribeToTextQuestEvents,
  waitForNextTextQuestEvent,
} from "../src/index";

describe("TextQuest WebSocket helpers", () => {
  const server = createServer();
  const wsServer = new WebSocketServer({ noServer: true });
  let baseHttpUrl: string;

  function makeWebSocket(url: string): TextQuestWebSocketLike {
    return new WebSocket(url) as unknown as TextQuestWebSocketLike;
  }

  server.on("upgrade", (request, socket, head) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    const token = url.searchParams.get("token");

    if (token !== "secret-token") {
      socket.write("HTTP/1.1 401 Unauthorized\r\n\r\n");
      socket.destroy();
      return;
    }

    wsServer.handleUpgrade(request, socket, head, (ws) => {
      ws.send(JSON.stringify({ type: "dashboard.snapshot", ok: true }));
      ws.on("message", (message: RawData) => {
        ws.send(String(message));
      });
    });
  });

  beforeAll(async () => {
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
    const address = server.address() as AddressInfo;
    baseHttpUrl = `http://127.0.0.1:${address.port}`;
  });

  afterAll(async () => {
    wsServer.clients.forEach((client) => client.terminate());
    wsServer.close();
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  });

  it("builds websocket URLs with token query parameters", () => {
    expect(
      createTextQuestWebSocketUrl({
        baseUrl: "https://example.test/root/",
        apiToken: "secret-token",
      }),
    ).toBe("wss://example.test/ws?token=secret-token");
  });

  it("subscribes with query token auth and receives parsed JSON events", async () => {
    const events: unknown[] = [];
    const subscription = await subscribeToTextQuestEvents({
      baseUrl: baseHttpUrl,
      apiToken: "secret-token",
      webSocketFactory: makeWebSocket,
      onMessage: (event) => events.push(event.json()),
    });

    await subscription.sendJson({ type: "ping" });

    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(events).toContainEqual({ type: "dashboard.snapshot", ok: true });
    expect(events).toContainEqual({ type: "ping" });

    subscription.close();
  });

  it("surfaces auth failures as websocket errors", async () => {
    await expect(
      subscribeToTextQuestEvents({
        baseUrl: baseHttpUrl,
        webSocketFactory: makeWebSocket,
      }),
    ).rejects.toBeInstanceOf(TextQuestWebSocketError);
  });

  it("waits for the next websocket event", async () => {
    const event = await waitForNextTextQuestEvent({
      baseUrl: baseHttpUrl,
      apiToken: "secret-token",
      webSocketFactory: makeWebSocket,
    });

    expect(event.json()).toEqual({ type: "dashboard.snapshot", ok: true });
  });
});
