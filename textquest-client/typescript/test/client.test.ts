import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";

import {
  TextQuestApiError,
  TextQuestClient,
  type BoxChatSettings,
  type Session,
} from "../src/index";

const sessions: Session[] = [
  {
    client_id: 1,
    character_name: "Frostreaver",
    zone: "South Karana",
    level: 60,
    hp_pct: 95.5,
    mana_pct: 87,
    status: "idle",
  },
];

const boxChatSettings: BoxChatSettings = {
  enabled: true,
  host: "127.0.0.1",
  port: 2112,
  auto_connect: true,
};

describe("TextQuestClient", () => {
  let requestCount = 0;
  let unauthorizedCount = 0;
  let flakyCount = 0;
  const server = createServer(async (req, res) => {
    requestCount += 1;

    const body = await new Promise<string>((resolve) => {
      let chunked = "";
      req.on("data", (chunk) => {
        chunked += String(chunk);
      });
      req.on("end", () => resolve(chunked));
    });

    if (req.url === "/api/sessions" && req.method === "GET") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify(sessions));
      return;
    }

    if (req.url === "/api/box-chat/settings" && req.method === "PUT") {
      const token = req.headers["x-api-token"];
      if (token !== "secret-token") {
        unauthorizedCount += 1;
        res.writeHead(401, { "content-type": "application/json" });
        res.end(JSON.stringify({ error: "Missing or invalid X-API-Token" }));
        return;
      }

      res.writeHead(200, { "content-type": "application/json" });
      res.end(body);
      return;
    }

    if (req.url === "/api/raid/config" && req.method === "GET") {
      res.writeHead(501, { "content-type": "application/json" });
      res.end(
        JSON.stringify({
          error: "Raid configuration API is not implemented in this build",
        }),
      );
      return;
    }

    if (req.url === "/api/missing" && req.method === "GET") {
      res.writeHead(404, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: "API route not found" }));
      return;
    }

    if (req.url === "/api/flaky" && req.method === "GET") {
      flakyCount += 1;
      if (flakyCount < 3) {
        res.writeHead(503, { "content-type": "application/json" });
        res.end(JSON.stringify({ error: "Temporarily unavailable" }));
        return;
      }

      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true, attempt: flakyCount }));
      return;
    }

    if (req.url === "/api/slow" && req.method === "GET") {
      await new Promise((resolve) => setTimeout(resolve, 100));
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
      return;
    }

    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "Unhandled route" }));
  });

  let baseUrl: string;

  beforeAll(async () => {
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
    const address = server.address() as AddressInfo;
    baseUrl = `http://127.0.0.1:${address.port}`;
  });

  afterAll(async () => {
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve())),
    );
  });

  it("lists sessions with typed responses", async () => {
    const client = new TextQuestClient({ baseUrl });

    const response = await client.listSessions();

    expect(response).toEqual(sessions);
  });

  it("sends the configured API token on mutating requests", async () => {
    const client = new TextQuestClient({ baseUrl, apiToken: "secret-token" });

    const saved = await client.putBoxChatSettings(boxChatSettings);

    expect(saved).toEqual(boxChatSettings);
    expect(unauthorizedCount).toBe(0);
  });

  it("throws a decoded API error for 401 responses", async () => {
    const client = new TextQuestClient({ baseUrl });

    await expect(client.putBoxChatSettings(boxChatSettings)).rejects.toMatchObject({
      name: "TextQuestApiError",
      status: 401,
      payload: { error: "Missing or invalid X-API-Token" },
    });
  });

  it("throws a decoded API error for 404 responses", async () => {
    const client = new TextQuestClient({ baseUrl });

    await expect(client.request({ path: "/api/missing", method: "GET" })).rejects.toMatchObject({
      name: "TextQuestApiError",
      status: 404,
      payload: { error: "API route not found" },
    });
  });

  it("throws a decoded API error for 501 responses", async () => {
    const client = new TextQuestClient({ baseUrl });

    await expect(client.getRaidConfig()).rejects.toMatchObject({
      name: "TextQuestApiError",
      status: 501,
      payload: {
        error: "Raid configuration API is not implemented in this build",
      },
    });
  });

  it("retries configured transient failures", async () => {
    const client = new TextQuestClient({
      baseUrl,
      retry: {
        maxAttempts: 3,
        retryDelayMs: 1,
        retryOnStatuses: [503],
      },
    });

    const response = await client.request<{ ok: boolean; attempt: number }>({
      path: "/api/flaky",
      method: "GET",
    });

    expect(response).toEqual({ ok: true, attempt: 3 });
  });

  it("times out slow requests", async () => {
    const client = new TextQuestClient({
      baseUrl,
      timeoutMs: 10,
      retry: { maxAttempts: 1 },
    });

    await expect(client.request({ path: "/api/slow", method: "GET" })).rejects.toMatchObject({
      name: "TextQuestApiError",
      code: "TIMEOUT",
    });
  });

  it("preserves method and URL on decoded API errors", async () => {
    const client = new TextQuestClient({ baseUrl });

    let error: unknown;
    try {
      await client.request({ path: "/api/missing", method: "GET" });
    } catch (caught) {
      error = caught;
    }

    expect(error).toBeInstanceOf(TextQuestApiError);
    expect(error).toMatchObject({
      method: "GET",
      url: `${baseUrl}/api/missing`,
    });
  });
});
