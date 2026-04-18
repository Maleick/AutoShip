import type {
  SessionInfo,
  HealthResponse,
  ErrorResponse,
  CommandResponse,
  GroupAssignment,
  ClientConfig,
  SessionEvent,
} from './types.js';
import { TextQuestClientError } from './types.js';

export class TextQuestClient {
  private baseUrl: string;
  private apiToken?: string;
  private timeoutMs: number;

  constructor(config: ClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, '');
    this.apiToken = config.apiToken;
    this.timeoutMs = config.timeoutMs ?? 30000;
  }

  private buildUrl(path: string): string {
    return `${this.baseUrl}${path}`;
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown
  ): Promise<T> {
    const url = this.buildUrl(path);
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.apiToken) {
      headers['X-API-Token'] = this.apiToken;
    }

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeoutMs);

    try {
      const response = await fetch(url, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        const error: ErrorResponse = await response
          .json()
          .catch(() => ({ error: 'Unknown error' }));
        throw new TextQuestClientError(error.error, response.status, error);
      }

      if (response.status === 204) {
        return undefined as T;
      }

      return response.json() as Promise<T>;
    } catch (e) {
      clearTimeout(timeoutId);
      if (e instanceof TextQuestClientError) {
        throw e;
      }
      if (e instanceof Error && e.name === 'AbortError') {
        throw new TextQuestClientError(
          `Request timeout after ${this.timeoutMs}ms`
        );
      }
      throw new TextQuestClientError(e instanceof Error ? e.message : 'Unknown error');
    }
  }

  async health(): Promise<HealthResponse> {
    return this.request<HealthResponse>('GET', '/api/health');
  }

  async listSessions(): Promise<SessionInfo[]> {
    return this.request<SessionInfo[]>('GET', '/api/sessions');
  }

  async pauseSession(sessionId: number): Promise<void> {
    return this.request<void>('PUT', `/api/control/pause/${sessionId}`);
  }

  async resumeSession(sessionId: number): Promise<void> {
    return this.request<void>('PUT', `/api/control/resume/${sessionId}`);
  }

  async setSessionGroup(sessionId: number, groupId: number): Promise<void> {
    return this.request<void>('PUT', `/api/control/group/${sessionId}`, {
      group_id: groupId,
    } as GroupAssignment);
  }

  async broadcastAll(sessionId: number): Promise<void> {
    return this.request<void>('PUT', `/api/control/broadcast-all/${sessionId}`);
  }

  async relayCommand(command: string, target?: string): Promise<string> {
    const response = await this.request<CommandResponse>('POST', '/api/command', {
      command,
      target,
    });
    return response.message;
  }
}
