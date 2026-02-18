import * as vscode from 'vscode';
import { ChildProcess, spawn } from 'child_process';
import * as net from 'net';
import * as http from 'http';
import * as path from 'path';
import * as fs from 'fs';

/**
 * Manages the but-server sidecar process.
 * Spawns it on a random free port, monitors health, and kills it on dispose.
 */
export class ServerManager implements vscode.Disposable {
  private process: ChildProcess | null = null;
  private _port: number = 0;
  private _isRunning = false;
  private outputChannel: vscode.OutputChannel;
  private extensionPath: string;

  get port(): number {
    return this._port;
  }

  get baseUrl(): string {
    return `http://127.0.0.1:${this._port}`;
  }

  get wsUrl(): string {
    return `ws://127.0.0.1:${this._port}/ws`;
  }

  get isRunning(): boolean {
    return this._isRunning;
  }

  constructor(outputChannel: vscode.OutputChannel, extensionPath: string) {
    this.outputChannel = outputChannel;
    this.extensionPath = extensionPath;
  }

  /**
   * Find a free port by binding to port 0 and reading the assigned port.
   */
  private async findFreePort(): Promise<number> {
    return new Promise((resolve, reject) => {
      const server = net.createServer();
      server.listen(0, '127.0.0.1', () => {
        const addr = server.address();
        if (addr && typeof addr === 'object') {
          const port = addr.port;
          server.close(() => resolve(port));
        } else {
          server.close(() => reject(new Error('Failed to get port')));
        }
      });
      server.on('error', reject);
    });
  }

  /**
   * Resolve the path to the but-server binary.
   * Priority: 1) user config, 2) bundled binary in extension, 3) PATH fallback.
   */
  private getServerBinaryPath(): string {
    const config = vscode.workspace.getConfiguration('gitbutler');
    const configPath = config.get<string>('serverPath');
    if (configPath && configPath !== 'but-server') {
      this.outputChannel.appendLine(`Using configured server path: ${configPath}`);
      return configPath;
    }

    // Check for bundled binary inside the extension
    const bundledName = process.platform === 'win32' ? 'but-server.exe' : 'but-server';
    const bundledPath = path.join(this.extensionPath, 'bin', bundledName);
    this.outputChannel.appendLine(`Extension path: ${this.extensionPath}`);
    this.outputChannel.appendLine(`Looking for bundled binary at: ${bundledPath}`);
    const exists = fs.existsSync(bundledPath);
    this.outputChannel.appendLine(`Bundled binary exists: ${exists}`);
    if (exists) {
      return bundledPath;
    }

    // Fallback: assume `but-server` is on PATH
    this.outputChannel.appendLine('Falling back to PATH lookup for but-server');
    return 'but-server';
  }

  /**
   * Start the but-server process on a random free port.
   */
  async start(): Promise<void> {
    if (this._isRunning) {
      this.outputChannel.appendLine('Server already running');
      return;
    }

    this._port = await this.findFreePort();
    const binaryPath = this.getServerBinaryPath();

    this.outputChannel.appendLine(`Starting but-server on port ${this._port}...`);
    this.outputChannel.appendLine(`Binary: ${binaryPath}`);

    this.process = spawn(binaryPath, [], {
      env: {
        ...process.env,
        BUTLER_PORT: String(this._port),
        BUTLER_HOST: '127.0.0.1',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true,
    });

    // Pipe stdout/stderr to output channel
    this.process.stdout?.on('data', (data: Buffer) => {
      this.outputChannel.appendLine(`[server] ${data.toString().trim()}`);
    });

    this.process.stderr?.on('data', (data: Buffer) => {
      this.outputChannel.appendLine(`[server:err] ${data.toString().trim()}`);
    });

    this.process.on('error', (err) => {
      this.outputChannel.appendLine(`Server process error: ${err.message}`);
      this._isRunning = false;
    });

    this.process.on('exit', (code, signal) => {
      this.outputChannel.appendLine(
        `Server exited (code=${code}, signal=${signal})`
      );
      this._isRunning = false;
      this.process = null;
    });

    // Wait for the server to be ready by polling the port
    await this.waitForReady(10000);
    this._isRunning = true;
    this.outputChannel.appendLine(`Server ready at ${this.baseUrl}`);
  }

  /**
   * Poll the server until it responds or timeout.
   */
  private async waitForReady(timeoutMs: number): Promise<void> {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      try {
        await this.healthCheck();
        return;
      } catch {
        await new Promise((r) => setTimeout(r, 200));
      }
    }
    throw new Error(`Server did not become ready within ${timeoutMs}ms`);
  }

  /**
   * Simple health check — try to connect to the port.
   */
  private healthCheck(): Promise<void> {
    return new Promise((resolve, reject) => {
      const req = http.request(
        {
          hostname: '127.0.0.1',
          port: this._port,
          path: '/build_type',
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          timeout: 2000,
        },
        (res) => {
          res.resume();
          resolve();
        }
      );
      req.on('error', reject);
      req.on('timeout', () => {
        req.destroy();
        reject(new Error('timeout'));
      });
      req.write('{}');
      req.end();
    });
  }

  /**
   * Stop the server process.
   */
  stop(): void {
    if (this.process) {
      this.outputChannel.appendLine('Stopping but-server...');
      this.process.kill('SIGTERM');
      // Force kill after 3 seconds if it doesn't exit
      setTimeout(() => {
        if (this.process && !this.process.killed) {
          this.process.kill('SIGKILL');
        }
      }, 3000);
      this.process = null;
      this._isRunning = false;
    }
  }

  dispose(): void {
    this.stop();
  }
}
