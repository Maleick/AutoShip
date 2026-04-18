import type {
  Account,
  CreateAccountPayload,
  UpdateAccountPayload,
  Session,
  AdminSessionRecord,
} from "../types";

/**
 * Type-safe API error with HTTP status and message
 */
export class ApiError extends Error {
  constructor(
    public status: number,
    public statusText: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/**
 * Generic API response wrapper with error handling
 */
export interface ApiResponse<T> {
  data?: T;
  error?: string;
  status: number;
}

/**
 * Base API configuration
 */
export interface ApiConfig {
  baseUrl?: string;
  timeout?: number;
}

/**
 * API Client for TextQuest backend
 *
 * Provides type-safe methods for:
 * - Credentials (Accounts) API
 * - Sessions API
 * - Groups API
 * - Camps API
 */
export class ApiClient {
  private baseUrl: string;
  private timeout: number;

  constructor(config: ApiConfig = {}) {
    this.baseUrl = config.baseUrl || "";
    this.timeout = config.timeout || 30000;
  }

  /**
   * Make a typed HTTP request with error handling
   */
  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const options: RequestInit = {
        method,
        signal: controller.signal,
        headers: {
          "Content-Type": "application/json",
        },
      };

      if (body !== undefined) {
        options.body = JSON.stringify(body);
      }

      const response = await fetch(url, options);
      clearTimeout(timeoutId);

      if (!response.ok) {
        let errorMessage: string;
        try {
          const errorBody = await response.json();
          errorMessage = errorBody.error || response.statusText;
        } catch {
          errorMessage = response.statusText;
        }
        throw new ApiError(response.status, response.statusText, errorMessage);
      }

      // Handle empty responses (204 No Content, etc.)
      if (response.status === 204) {
        return undefined as T;
      }

      return response.json();
    } catch (error) {
      clearTimeout(timeoutId);
      if (error instanceof ApiError) {
        throw error;
      }
      if (error instanceof TypeError) {
        throw new ApiError(0, "Network Error", error.message);
      }
      throw error;
    }
  }

  // ── Credentials / Accounts API ──────────────────────────────────────────

  /**
   * List all accounts
   */
  async listAccounts(): Promise<Account[]> {
    return this.request<Account[]>("GET", "/api/accounts");
  }

  /**
   * Get a single account by name
   */
  async getAccount(name: string): Promise<Account> {
    return this.request<Account>(
      "GET",
      `/api/accounts/${encodeURIComponent(name)}`,
    );
  }

  /**
   * Create a new account
   */
  async createAccount(payload: CreateAccountPayload): Promise<Account> {
    return this.request<Account>("POST", "/api/accounts", payload);
  }

  /**
   * Update an existing account
   */
  async updateAccount(
    name: string,
    payload: UpdateAccountPayload,
  ): Promise<Account> {
    return this.request<Account>(
      "PUT",
      `/api/accounts/${encodeURIComponent(name)}`,
      payload,
    );
  }

  /**
   * Delete an account
   */
  async deleteAccount(name: string): Promise<void> {
    await this.request<void>(
      "DELETE",
      `/api/accounts/${encodeURIComponent(name)}`,
    );
  }

  /**
   * Set or update account password
   */
  async setAccountPassword(name: string, password: string): Promise<void> {
    await this.request<void>(
      "PUT",
      `/api/accounts/${encodeURIComponent(name)}/password`,
      { password },
    );
  }

  /**
   * Export all accounts as JSON
   */
  async exportAccounts(): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>("GET", "/api/accounts/export");
  }

  /**
   * Import accounts from JSON payload
   */
  async importAccounts(
    accounts: CreateAccountPayload[],
  ): Promise<{ imported: number }> {
    return this.request<{ imported: number }>("POST", "/api/accounts/import", {
      accounts,
    });
  }

  // ── Sessions API ────────────────────────────────────────────────────────

  /**
   * List all active sessions
   */
  async listSessions(): Promise<Session[]> {
    return this.request<Session[]>("GET", "/api/sessions");
  }

  /**
   * Get a single session by client ID
   */
  async getSession(clientId: number): Promise<Session> {
    return this.request<Session>("GET", `/api/sessions/${clientId}`);
  }

  /**
   * List all admin session records (detailed metadata)
   */
  async listAdminSessions(): Promise<AdminSessionRecord[]> {
    return this.request<AdminSessionRecord[]>("GET", "/api/admin/sessions");
  }

  /**
   * Get a specific admin session record
   */
  async getAdminSession(sessionId: string): Promise<AdminSessionRecord> {
    return this.request<AdminSessionRecord>(
      "GET",
      `/api/admin/sessions/${encodeURIComponent(sessionId)}`,
    );
  }

  /**
   * Start a session (typically initiates automation for a character)
   */
  async startSession(sessionId: string): Promise<AdminSessionRecord> {
    return this.request<AdminSessionRecord>(
      "POST",
      `/api/admin/sessions/${encodeURIComponent(sessionId)}/start`,
      {},
    );
  }

  /**
   * Stop a session (halts automation)
   */
  async stopSession(sessionId: string): Promise<AdminSessionRecord> {
    return this.request<AdminSessionRecord>(
      "POST",
      `/api/admin/sessions/${encodeURIComponent(sessionId)}/stop`,
      {},
    );
  }

  // ── Groups API ──────────────────────────────────────────────────────────

  /**
   * List all groups
   */
  async listGroups(): Promise<Record<string, unknown>[]> {
    return this.request<Record<string, unknown>[]>("GET", "/api/groups");
  }

  /**
   * Get a single group by ID
   */
  async getGroup(groupId: string): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>(
      "GET",
      `/api/groups/${encodeURIComponent(groupId)}`,
    );
  }

  /**
   * Create a new group
   */
  async createGroup(
    payload: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>(
      "POST",
      "/api/groups",
      payload,
    );
  }

  /**
   * Update an existing group
   */
  async updateGroup(
    groupId: string,
    payload: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>(
      "PUT",
      `/api/groups/${encodeURIComponent(groupId)}`,
      payload,
    );
  }

  /**
   * Delete a group
   */
  async deleteGroup(groupId: string): Promise<void> {
    await this.request<void>(
      "DELETE",
      `/api/groups/${encodeURIComponent(groupId)}`,
    );
  }

  // ── Camps API ───────────────────────────────────────────────────────────

  /**
   * List all camp configurations
   */
  async listCamps(): Promise<Record<string, unknown>[]> {
    return this.request<Record<string, unknown>[]>("GET", "/api/camps");
  }

  /**
   * Get a single camp by ID
   */
  async getCamp(campId: string): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>(
      "GET",
      `/api/camps/${encodeURIComponent(campId)}`,
    );
  }

  /**
   * Create a new camp
   */
  async createCamp(
    payload: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>("POST", "/api/camps", payload);
  }

  /**
   * Update an existing camp
   */
  async updateCamp(
    campId: string,
    payload: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return this.request<Record<string, unknown>>(
      "PUT",
      `/api/camps/${encodeURIComponent(campId)}`,
      payload,
    );
  }
}

/**
 * Default singleton instance with standard configuration
 */
export const apiClient = new ApiClient();

/**
 * Helper functions for common API operations
 * (for backwards compatibility with hook-based patterns)
 */

export async function fetchAccountsList(): Promise<Account[]> {
  return apiClient.listAccounts();
}

export async function fetchSessionsList(): Promise<Session[]> {
  return apiClient.listSessions();
}

export async function fetchGroupsList(): Promise<Record<string, unknown>[]> {
  return apiClient.listGroups();
}

export async function fetchCampsList(): Promise<Record<string, unknown>[]> {
  return apiClient.listCamps();
}
