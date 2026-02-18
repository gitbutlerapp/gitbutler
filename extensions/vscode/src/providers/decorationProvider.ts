import * as vscode from 'vscode';
import * as path from 'path';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';
import type { FileChange } from '../types/gitbutler';

/**
 * Provides file decorations in the Explorer tree.
 * Shows colored badges (M, A, D, R, U) next to changed files.
 */
export class GitButlerDecorationProvider implements vscode.FileDecorationProvider, vscode.Disposable {
  private _onDidChangeFileDecorations = new vscode.EventEmitter<vscode.Uri | vscode.Uri[] | undefined>();
  readonly onDidChangeFileDecorations = this._onDidChangeFileDecorations.event;

  private disposable: vscode.Disposable;
  private changeMap = new Map<string, FileChange>();

  constructor(private readonly workspace: WorkspaceService) {
    this.disposable = workspace.onDidChangeState((state) => this.updateState(state));
  }

  private updateState(state: WorkspaceState): void {
    this.changeMap.clear();

    const allChanges = [...state.changes, ...state.stagedChanges];
    for (const change of allChanges) {
      this.changeMap.set(change.path, change);
    }

    // Fire a global update
    this._onDidChangeFileDecorations.fire(undefined);
  }

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri);
    if (!workspaceFolder) return undefined;

    const relativePath = path.relative(workspaceFolder.uri.fsPath, uri.fsPath).replace(/\\/g, '/');
    const change = this.changeMap.get(relativePath);
    if (!change) return undefined;

    switch (change.type) {
      case 'added':
        return {
          badge: 'A',
          color: new vscode.ThemeColor('gitbutler.addedForeground'),
          tooltip: 'Added (GitButler)',
        };
      case 'untracked':
        return {
          badge: 'U',
          color: new vscode.ThemeColor('gitbutler.addedForeground'),
          tooltip: 'Untracked (GitButler)',
        };
      case 'modified':
        return {
          badge: 'M',
          color: new vscode.ThemeColor('gitbutler.modifiedForeground'),
          tooltip: 'Modified (GitButler)',
        };
      case 'deleted':
        return {
          badge: 'D',
          color: new vscode.ThemeColor('gitbutler.deletedForeground'),
          tooltip: 'Deleted (GitButler)',
        };
      case 'renamed':
        return {
          badge: 'R',
          color: new vscode.ThemeColor('gitbutler.renamedForeground'),
          tooltip: `Renamed${change.previousPath ? ` from ${change.previousPath}` : ''} (GitButler)`,
        };
      default:
        return undefined;
    }
  }

  dispose(): void {
    this.disposable.dispose();
    this._onDidChangeFileDecorations.dispose();
  }
}
