import * as vscode from 'vscode';
import { ServerManager } from './services/server';
import { ButApiClient } from './services/api';
import { EventService } from './services/events';
import { WorkspaceService } from './services/workspace';
import { GitButlerDecorationProvider } from './providers/decorationProvider';
import { GitButlerWebviewProvider } from './views/webviewPanel';
import { StatusBarService } from './views/statusBar';
import { registerCommands } from './commands';

let outputChannel: vscode.OutputChannel;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('GitButler');
  outputChannel.appendLine('GitButler extension activating...');

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (!workspaceFolder) {
    outputChannel.appendLine('No workspace folder found');
    registerFallbackCommands(context);
    return;
  }

  const workDir = workspaceFolder.uri.fsPath;
  outputChannel.appendLine(`Workspace: ${workDir}`);

  // -------------------------------------------------------------------
  // Start but-server as a sidecar on a random free port
  // -------------------------------------------------------------------
  const server = new ServerManager(outputChannel, context.extensionPath);
  context.subscriptions.push(server);

  try {
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Window,
        title: 'GitButler: Starting server...',
      },
      () => server.start()
    );
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    outputChannel.appendLine(`Failed to start but-server: ${msg}`);
    registerFallbackCommands(context);
    vscode.window
      .showWarningMessage(
        `GitButler: Could not start but-server. Is it installed?\n\n${msg}`,
        'Open Settings',
        'Install GitButler'
      )
      .then((choice) => {
        if (choice === 'Open Settings') {
          vscode.commands.executeCommand('workbench.action.openSettings', 'gitbutler.serverPath');
        } else if (choice === 'Install GitButler') {
          vscode.env.openExternal(vscode.Uri.parse('https://gitbutler.com/downloads'));
        }
      });
    return;
  }

  outputChannel.appendLine(`Server running on port ${server.port}`);

  // -------------------------------------------------------------------
  // Create API client and register the project
  // -------------------------------------------------------------------
  const api = new ButApiClient(server, outputChannel);
  context.subscriptions.push(api);

  let projectId: string;
  try {
    projectId = await api.registerProject(workDir);
    outputChannel.appendLine(`Project ID: ${projectId}`);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    outputChannel.appendLine(`Failed to register project: ${msg}`);
    registerFallbackCommands(context);
    vscode.window.showErrorMessage(`GitButler: Failed to register project — ${msg}`);
    return;
  }

  // -------------------------------------------------------------------
  // Check operating mode (don't auto-enter workspace mode)
  // -------------------------------------------------------------------
  try {
    const mode = await api.operatingMode();
    outputChannel.appendLine(`Operating mode: ${mode.operatingMode.type}`);
    if (mode.operatingMode.type === 'OutsideWorkspace') {
      outputChannel.appendLine('Repo is outside workspace mode — virtual branch features require workspace mode');
      outputChannel.appendLine('Use "GitButler: Setup" command to enter workspace mode');
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    outputChannel.appendLine(`Warning: Could not check operating mode: ${msg}`);
  }

  // -------------------------------------------------------------------
  // Connect WebSocket for real-time events
  // -------------------------------------------------------------------
  const events = new EventService(server, outputChannel, projectId);
  context.subscriptions.push(events);
  events.connect();

  // -------------------------------------------------------------------
  // Initialize workspace service (HTTP API + WebSocket events)
  // -------------------------------------------------------------------
  const workspace = new WorkspaceService(api, events);
  context.subscriptions.push(workspace);

  // -------------------------------------------------------------------
  // Create webview panel in Source Control sidebar
  // -------------------------------------------------------------------
  const webviewProvider = new GitButlerWebviewProvider(workspace, api, context.extensionUri);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider('gitbutlerPanel', webviewProvider)
  );

  // -------------------------------------------------------------------
  // File decoration provider
  // -------------------------------------------------------------------
  const decorationProvider = new GitButlerDecorationProvider(workspace);
  context.subscriptions.push(vscode.window.registerFileDecorationProvider(decorationProvider));

  // -------------------------------------------------------------------
  // Status bar
  // -------------------------------------------------------------------
  const statusBar = new StatusBarService(workspace);
  context.subscriptions.push(statusBar);

  // -------------------------------------------------------------------
  // Register commands
  // -------------------------------------------------------------------
  registerCommands(context, api, workspace);

  // -------------------------------------------------------------------
  // Enable UI context
  // -------------------------------------------------------------------
  vscode.commands.executeCommand('setContext', 'gitbutler.enabled', true);

  // -------------------------------------------------------------------
  // Initial data load
  // -------------------------------------------------------------------
  await workspace.initialize();
  outputChannel.appendLine('GitButler extension activated successfully');

  // -------------------------------------------------------------------
  // Config change listener
  // -------------------------------------------------------------------
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('gitbutler')) {
        outputChannel.appendLine('Configuration changed — reload window to apply');
      }
    })
  );
}

function registerFallbackCommands(context: vscode.ExtensionContext): void {
  const unavailable = () => {
    vscode.window.showErrorMessage('GitButler: Server is not running.');
  };
  const commands = [
    'gitbutler.refresh',
    'gitbutler.commit',
    'gitbutler.commitToBranch',
    'gitbutler.stage',
    'gitbutler.unstage',
    'gitbutler.discard',
    'gitbutler.openFile',
    'gitbutler.openDiff',
    'gitbutler.createBranch',
    'gitbutler.deleteBranch',
    'gitbutler.push',
    'gitbutler.pull',
    'gitbutler.undo',
    'gitbutler.stageFile',
    'gitbutler.unstageFile',
    'gitbutler.discardFile',
    'gitbutler.stageAll',
    'gitbutler.unstageAll',
    'gitbutler.showBranchCommits',
    'gitbutler.pushBranch',
    'gitbutler.commitToStack',
    'gitbutler.generateCommitMessage',
    'gitbutler.setup',
    'gitbutler.moveToBranch',
    'gitbutler.openInGUI',
  ];
  for (const cmd of commands) {
    context.subscriptions.push(vscode.commands.registerCommand(cmd, unavailable));
  }
}

/**
 * Detect the default remote tracking branch for the repo (e.g. "origin/main" or "origin/master").
 */
async function detectBaseBranch(workDir: string, output: vscode.OutputChannel): Promise<string | null> {
  const { execSync } = await import('child_process');
  try {
    // Try HEAD's upstream first
    const upstream = execSync('git rev-parse --abbrev-ref @{upstream}', {
      cwd: workDir,
      encoding: 'utf-8',
      timeout: 5000,
    }).trim();
    if (upstream) return upstream;
  } catch {
    // No upstream set
  }

  try {
    // Check for origin/main or origin/master
    const refs = execSync('git remote show origin -n', {
      cwd: workDir,
      encoding: 'utf-8',
      timeout: 5000,
    });
    const headMatch = refs.match(/HEAD branch:\s*(\S+)/);
    if (headMatch) {
      return `origin/${headMatch[1]}`;
    }
  } catch {
    // No origin remote
  }

  try {
    // Last resort: check if origin/main or origin/master exists
    const branches = execSync('git branch -r', {
      cwd: workDir,
      encoding: 'utf-8',
      timeout: 5000,
    });
    if (branches.includes('origin/main')) return 'origin/main';
    if (branches.includes('origin/master')) return 'origin/master';
  } catch {
    // ignore
  }

  return null;
}

export function deactivate(): void {
  outputChannel?.appendLine('GitButler extension deactivated');
  // Server process is killed via dispose() on the ServerManager
}
