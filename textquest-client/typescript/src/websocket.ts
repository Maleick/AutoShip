import type { SessionEvent, ClientConfig } from './types.js';
import { TextQuestClientError } from './types.js';

type EventCallback = (event: SessionEvent) => void;

export class TextQuestWebSocket {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelayMs = 1000;
  private eventCallback?: EventCallback;
  private url: string;

  constructor(private config: ClientConfig) {
    this.url = config.baseUrl.replace('http', 'ws').replace(/\/$/, '') + '/ws';
  }

  private buildAuthUrl(): string {
    if (this.config.apiToken) {
      return `${this.url}?token=${encodeURIComponent(this.config.apiToken)}`;
    }
    return this.url;
  }

  async connect(onEvent: EventCallback): Promise<void> {
    return new Promise((resolve, reject) => {
      this.eventCallback = onEvent;

      try {
        this.ws = new WebSocket(this.buildAuthUrl());

        this.ws.onopen = () => {
          console.log('WebSocket connected');
          this.reconnectAttempts = 0;
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data);
            const sessionEvent: SessionEvent = this.parseEvent(data);
            if (this.eventCallback) {
              this.eventCallback(sessionEvent);
            }
          } catch (e) {
            console.error('Failed to parse WebSocket message:', e);
          }
        };

        this.ws.onclose = () => {
          console.log('WebSocket disconnected');
          this.attemptReconnect();
        };

        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error);
          reject(new TextQuestClientError('WebSocket connection failed'));
        };
      } catch (e) {
        reject(e);
      }
    });
  }

  private parseEvent(data: unknown): SessionEvent {
    const obj = data as Record<string, unknown>;
    const type = (obj.type as string) || 'unknown';

    switch (type) {
      case 'session':
      case 'session_update':
        return {
          type: 'session_update',
          session: obj.session as any,
        };
      case 'command':
        return {
          type: 'command',
          command: obj.command as string,
          target: obj.target as string | undefined,
        };
      default:
        return {
          type: 'error',
          message: `Unknown event type: ${type}`,
        };
    }
  }

  private async attemptReconnect(): Promise<void> {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnect attempts reached');
      return;
    }

    this.reconnectAttempts++;
    const delay = this.reconnectDelayMs * Math.pow(2, this.reconnectAttempts - 1);
    console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})...`);

    await new Promise((resolve) => setTimeout(resolve, delay));

    if (this.eventCallback) {
      try {
        await this.connect(this.eventCallback);
      } catch (e) {
        console.error('Reconnect failed:', e);
      }
    }
  }

  send(command: string, target?: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new TextQuestClientError('WebSocket not connected');
    }

    const message = JSON.stringify({
      type: 'command',
      command,
      target,
    });
    this.ws.send(message);
  }

  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}