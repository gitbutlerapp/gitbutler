import * as vscode from 'vscode';
import WebSocket from 'ws';
import type { ServerManager } from './server';

/**
 * WebSocket event listener for real-time change notifications from but-server.
 * Replaces polling: the server pushes events when files change, branches update, etc.
 */
export class EventService implements vscode.Disposable {
  private ws: WebSocket | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private isDisposed = false;

  private _onWorktreeChanged = new vscode.EventEmitter<void>();
  readonly onWorktreeChanged = this._onWorktreeChanged.event;

  private _onHeadChanged = new vscode.EventEmitter<void>();
  readonly onHeadChanged = this._onHeadChanged.event;

  private _onGitActivity = new vscode.EventEmitter<void>();
  readonly onGitActivity = this._onGitActivity.event;

  private _onAnyEvent = new vscode.EventEmitter<ServerEvent>();
  readonly onAnyEvent = this._onAnyEvent.event;

  constructor(
    private readonly server: ServerManager,
    private readonly outputChannel: vscode.OutputChannel,
    private readonly projectId: string
  ) {}

  /**
   * Connect to the WebSocket and start listening for events.
   */
  connect(): void {
    if (this.isDisposed) return;

    const url = this.server.wsUrl;
    this.outputChannel.appendLine(`Connecting WebSocket: ${url}`);

    try {
      this.ws = new WebSocket(url);
    } catch (err) {
      this.outputChannel.appendLine(`WebSocket creation error: ${err}`);
      this.scheduleReconnect();
      return;
    }

    this.ws.on('open', () => {
      this.outputChannel.appendLine('WebSocket connected');
    });

    this.ws.on('message', (data: WebSocket.Data) => {
      try {
        const event = JSON.parse(data.toString()) as ServerEvent;
        this.handleEvent(event);
      } catch (err) {
        this.outputChannel.appendLine(`WebSocket parse error: ${err}`);
      }
    });

    this.ws.on('close', (code, reason) => {
      this.outputChannel.appendLine(
        `WebSocket closed (code=${code}, reason=${reason.toString()})`
      );
      this.scheduleReconnect();
    });

    this.ws.on('error', (err) => {
      this.outputChannel.appendLine(`WebSocket error: ${err.message}`);
      // 'close' will fire after this, triggering reconnect
    });
  }

  private handleEvent(event: ServerEvent): void {
    const name = event.name || '';
    this._onAnyEvent.fire(event);

    // Event names follow the pattern: `project://{projectId}/{eventType}`
    // We only care about events for our project
    const projectPrefix = `project://${this.projectId}/`;
    if (name.startsWith(projectPrefix)) {
      const eventType = name.substring(projectPrefix.length);

      switch (eventType) {
        case 'worktree_changes':
        case 'git_index':
          this._onWorktreeChanged.fire();
          break;
        case 'git_head':
          this._onHeadChanged.fire();
          break;
        case 'git_activity':
        case 'git_fetch':
          this._onGitActivity.fire();
          break;
        default:
          // Other events: still useful to log
          this.outputChannel.appendLine(`Event: ${eventType}`);
          break;
      }
    } else if (!name.startsWith('project://')) {
      // Global events (not project-scoped)
      this.outputChannel.appendLine(`Global event: ${name}`);
    }
  }

  private scheduleReconnect(): void {
    if (this.isDisposed) return;
    if (this.reconnectTimer) return;

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (!this.isDisposed && this.server.isRunning) {
        this.connect();
      }
    }, 3000);
  }

  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  dispose(): void {
    this.isDisposed = true;
    this.disconnect();
    this._onWorktreeChanged.dispose();
    this._onHeadChanged.dispose();
    this._onGitActivity.dispose();
    this._onAnyEvent.dispose();
  }
}

export interface ServerEvent {
  name: string;
  payload: unknown;
}
