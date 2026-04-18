import { describe, it, expect, beforeEach, vi } from "vitest";
import type {
  Account,
  CreateAccountPayload,
  UpdateAccountPayload,
  Session,
  AdminSessionRecord,
} from "../types";
import { ApiClient, ApiError } from "./api";
import { jsonResponse } from "../test/http";

// Mock fetch globally
global.fetch = vi.fn();

describe("ApiClient", () => {
  let client: ApiClient;
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    client = new ApiClient({ baseUrl: "http://localhost:3000" });
    mockFetch = global.fetch as ReturnType<typeof vi.fn>;
    mockFetch.mockClear();
  });

  describe("Error Handling", () => {
    it("should throw ApiError on HTTP error response", async () => {
      const mockResponse = new Response(JSON.stringify({ error: "Not found" }), {
        status: 404,
        statusText: "Not Found",
        headers: { "Content-Type": "application/json" },
      });
      mockFetch.mockResolvedValueOnce(mockResponse);

      try {
        await client.listAccounts();
        throw new Error("Should have thrown");
      } catch (e) {
        expect(e).toBeInstanceOf(ApiError);
      }
    });

    it("should handle network errors", async () => {
      mockFetch.mockRejectedValueOnce(new TypeError("Network error"));

      try {
        await client.listAccounts();
        throw new Error("Should have thrown");
      } catch (e) {
        expect(e).toBeInstanceOf(ApiError);
      }
    });

    it("should set timeout on requests", () => {
      // Verify that timeout is configurable
      const slowClient = new ApiClient({
        baseUrl: "http://localhost:3000",
        timeout: 50,
      });
      expect(slowClient).toBeDefined();
    });
  });

  describe("Accounts API", () => {
    const mockAccount: Account = {
      id: "1",
      name: "TestAccount",
      server: "Teek",
      character: "TestChar",
      class: "Warrior",
      group: 1,
      status: "active",
      has_password: true,
    };

    it("should list all accounts", async () => {
      mockFetch.mockResolvedValueOnce(
        jsonResponse([mockAccount], { status: 200 })
      );

      const accounts = await client.listAccounts();

      expect(accounts).toEqual([mockAccount]);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should get a single account", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse(mockAccount, { status: 200 }));

      const account = await client.getAccount("TestAccount");

      expect(account).toEqual(mockAccount);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/TestAccount",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should create an account", async () => {
      const payload: CreateAccountPayload = {
        name: "NewAccount",
        server: "Teek",
        character: "NewChar",
        class: "Wizard",
        group: 1,
        status: "active",
        password: "secret",
      };

      mockFetch.mockResolvedValueOnce(jsonResponse(mockAccount, { status: 200 }));

      const created = await client.createAccount(payload);

      expect(created).toEqual(mockAccount);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify(payload),
        })
      );
    });

    it("should update an account", async () => {
      const updatePayload: UpdateAccountPayload = {
        status: "locked",
      };

      const updatedAccount = { ...mockAccount, status: "locked" as const };
      mockFetch.mockResolvedValueOnce(
        jsonResponse(updatedAccount, { status: 200 })
      );

      const result = await client.updateAccount("TestAccount", updatePayload);

      expect(result).toEqual(updatedAccount);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/TestAccount",
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify(updatePayload),
        })
      );
    });

    it("should delete an account", async () => {
      mockFetch.mockResolvedValueOnce(
        new Response(JSON.stringify({}), { status: 200 })
      );

      await client.deleteAccount("TestAccount");

      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/TestAccount",
        expect.objectContaining({ method: "DELETE" })
      );
    });

    it("should set account password", async () => {
      mockFetch.mockResolvedValueOnce(
        new Response(JSON.stringify({}), { status: 200 })
      );

      await client.setAccountPassword("TestAccount", "newpassword");

      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/TestAccount/password",
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify({ password: "newpassword" }),
        })
      );
    });

    it("should export accounts", async () => {
      const exportData = { accounts: [mockAccount] };
      mockFetch.mockResolvedValueOnce(jsonResponse(exportData, { status: 200 }));

      const result = await client.exportAccounts();

      expect(result).toEqual(exportData);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/export",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should import accounts", async () => {
      const importPayload: CreateAccountPayload[] = [
        {
          name: "Account1",
          server: "Teek",
          character: "Char1",
          class: "Warrior",
          group: 1,
          status: "active",
        },
      ];

      mockFetch.mockResolvedValueOnce(
        jsonResponse({ imported: 1 }, { status: 200 })
      );

      const result = await client.importAccounts(importPayload);

      expect(result.imported).toBe(1);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/import",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ accounts: importPayload }),
        })
      );
    });
  });

  describe("Sessions API", () => {
    const mockSession: Session = {
      client_id: 1,
      character_name: "TestChar",
      zone: "Poknowledge",
      level: 65,
      hp_pct: 100,
      mana_pct: 100,
      status: "active",
      buff_count: 5,
      target_name: "goblin",
      target_hp_pct: 50,
      pet_name: "pet1",
    };

    const mockAdminSession: AdminSessionRecord = {
      sessionId: "session-1",
      characterName: "TestChar",
      profile: null,
      groupId: "group-1",
      routingScope: null,
      lifecycle: "active",
      status: "running",
      zone: "Poknowledge",
      level: 65,
      className: "Warrior",
      lastHeartbeat: new Date().toISOString(),
    };

    it("should list all sessions", async () => {
      mockFetch.mockResolvedValueOnce(
        jsonResponse([mockSession], { status: 200 })
      );

      const sessions = await client.listSessions();

      expect(sessions).toEqual([mockSession]);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/sessions",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should get a single session", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse(mockSession, { status: 200 }));

      const session = await client.getSession(1);

      expect(session).toEqual(mockSession);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/sessions/1",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should list admin sessions", async () => {
      mockFetch.mockResolvedValueOnce(
        jsonResponse([mockAdminSession], { status: 200 })
      );

      const sessions = await client.listAdminSessions();

      expect(sessions).toEqual([mockAdminSession]);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/admin/sessions",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should get a single admin session", async () => {
      mockFetch.mockResolvedValueOnce(
        jsonResponse(mockAdminSession, { status: 200 })
      );

      const session = await client.getAdminSession("session-1");

      expect(session).toEqual(mockAdminSession);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/admin/sessions/session-1",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should start a session", async () => {
      const startedSession = { ...mockAdminSession, lifecycle: "starting" };
      mockFetch.mockResolvedValueOnce(
        jsonResponse(startedSession, { status: 200 })
      );

      const result = await client.startSession("session-1");

      expect(result.lifecycle).toBe("starting");
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/admin/sessions/session-1/start",
        expect.objectContaining({ method: "POST" })
      );
    });

    it("should stop a session", async () => {
      const stoppedSession = { ...mockAdminSession, lifecycle: "stopping" };
      mockFetch.mockResolvedValueOnce(
        jsonResponse(stoppedSession, { status: 200 })
      );

      const result = await client.stopSession("session-1");

      expect(result.lifecycle).toBe("stopping");
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/admin/sessions/session-1/stop",
        expect.objectContaining({ method: "POST" })
      );
    });
  });

  describe("Groups API", () => {
    const mockGroup = {
      id: "group-1",
      name: "DPS Group",
      leader: "MainAssist",
      members: ["Wizard", "Magician", "Enchanter"],
    };

    it("should list all groups", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse([mockGroup], { status: 200 }));

      const groups = await client.listGroups();

      expect(groups).toEqual([mockGroup]);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/groups",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should get a single group", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse(mockGroup, { status: 200 }));

      const group = await client.getGroup("group-1");

      expect(group).toEqual(mockGroup);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/groups/group-1",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should create a group", async () => {
      const createPayload = {
        name: "New Group",
        leader: "MainAssist",
        members: [],
      };

      mockFetch.mockResolvedValueOnce(jsonResponse(mockGroup, { status: 201 }));

      const created = await client.createGroup(createPayload);

      expect(created).toEqual(mockGroup);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/groups",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify(createPayload),
        })
      );
    });

    it("should update a group", async () => {
      const updatePayload = {
        name: "Updated Group",
      };

      const updated = { ...mockGroup, name: "Updated Group" };
      mockFetch.mockResolvedValueOnce(jsonResponse(updated, { status: 200 }));

      const result = await client.updateGroup("group-1", updatePayload);

      expect(result.name).toBe("Updated Group");
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/groups/group-1",
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify(updatePayload),
        })
      );
    });

    it("should delete a group", async () => {
      mockFetch.mockResolvedValueOnce(new Response(JSON.stringify({}), { status: 200 }));

      await client.deleteGroup("group-1");

      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/groups/group-1",
        expect.objectContaining({ method: "DELETE" })
      );
    });
  });

  describe("Camps API", () => {
    const mockCamp = {
      id: "camp-1",
      name: "Poknowledge Spot 1",
      zone: "Poknowledge",
      x: 100,
      y: 200,
      z: 50,
      pull_radius: 30,
    };

    it("should list all camps", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse([mockCamp], { status: 200 }));

      const camps = await client.listCamps();

      expect(camps).toEqual([mockCamp]);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/camps",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should get a single camp", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse(mockCamp, { status: 200 }));

      const camp = await client.getCamp("camp-1");

      expect(camp).toEqual(mockCamp);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/camps/camp-1",
        expect.objectContaining({ method: "GET" })
      );
    });

    it("should create a camp", async () => {
      const createPayload = {
        name: "New Camp",
        zone: "Poknowledge",
        x: 150,
        y: 250,
        z: 60,
      };

      mockFetch.mockResolvedValueOnce(jsonResponse(mockCamp, { status: 201 }));

      const created = await client.createCamp(createPayload);

      expect(created).toEqual(mockCamp);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/camps",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify(createPayload),
        })
      );
    });

    it("should update a camp", async () => {
      const updatePayload = {
        pull_radius: 50,
      };

      const updated = { ...mockCamp, pull_radius: 50 };
      mockFetch.mockResolvedValueOnce(jsonResponse(updated, { status: 200 }));

      const result = await client.updateCamp("camp-1", updatePayload);

      expect(result.pull_radius).toBe(50);
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/camps/camp-1",
        expect.objectContaining({
          method: "PUT",
          body: JSON.stringify(updatePayload),
        })
      );
    });
  });

  describe("URL Encoding", () => {
    it("should URL encode special characters in paths", async () => {
      mockFetch.mockResolvedValueOnce(
        new Response(JSON.stringify({}), { status: 200 })
      );

      // Account names with spaces and special characters
      await client.deleteAccount("Test Account");

      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:3000/api/accounts/Test%20Account",
        expect.any(Object)
      );
    });
  });

  describe("Request Configuration", () => {
    it("should set correct Content-Type header", async () => {
      mockFetch.mockResolvedValueOnce(jsonResponse([], { status: 200 }));

      await client.listAccounts();

      const call = mockFetch.mock.calls[0];
      const options = call[1] as RequestInit;
      expect(options.headers).toBeDefined();
      const headers = options.headers as Record<string, string>;
      expect(headers["Content-Type"]).toBe("application/json");
    });

    it("should use configured base URL", async () => {
      const customClient = new ApiClient({ baseUrl: "https://api.example.com" });
      mockFetch.mockResolvedValueOnce(jsonResponse([], { status: 200 }));

      await customClient.listAccounts();

      expect(mockFetch).toHaveBeenCalledWith(
        "https://api.example.com/api/accounts",
        expect.any(Object)
      );
    });
  });

  describe("Helper Functions", () => {
    it("should provide backwards-compatible helper functions", async () => {
      const { fetchAccountsList, fetchSessionsList, fetchGroupsList, fetchCampsList } = await import('./api');
      
      expect(typeof fetchAccountsList).toBe("function");
      expect(typeof fetchSessionsList).toBe("function");
      expect(typeof fetchGroupsList).toBe("function");
      expect(typeof fetchCampsList).toBe("function");
    });
  });
});
