import * as vscode from 'vscode';
import * as path from 'path';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';
import type { FileChange } from '../types/gitbutler';

/**
 * Tree view showing unassigned (unstaged) worktree changes.
 * These are files not yet assigned to any branch.
 */
export class UnassignedTreeProvider implements vscode.TreeDataProvider<UnassignedTreeItem> {
  private _onDidChangeTreeData = new vscode.EventEmitter<UnassignedTreeItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private state: WorkspaceState | null = null;

  constructor(private readonly workspace: WorkspaceService) {
    workspace.onDidChangeState((state) => {
      this.state = state;
      this._onDidChangeTreeData.fire(undefined);
    });
  }

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  getTreeItem(element: UnassignedTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(): UnassignedTreeItem[] {
    if (!this.state?.isReady) {
      const item = new UnassignedTreeItem(
        'GitButler not initialized',
        vscode.TreeItemCollapsibleState.None,
        'info'
      );
      item.iconPath = new vscode.ThemeIcon('info');
      return [item];
    }

    const changes = this.state.changes;
    if (changes.length === 0) {
      const item = new UnassignedTreeItem(
        'No unassigned changes',
        vscode.TreeItemCollapsibleState.None,
        'info'
      );
      item.iconPath = new vscode.ThemeIcon('check');
      return [item];
    }

    return changes.map((change) => {
      const fileName = path.basename(change.path);
      const dirName = path.dirname(change.path);
      const item = new UnassignedTreeItem(
        fileName,
        vscode.TreeItemCollapsibleState.None,
        'file'
      );
      item.filePath = change.path;
      item.description = dirName !== '.' ? dirName : '';
      item.resourceUri = vscode.Uri.file(
        path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '', change.path)
      );
      item.iconPath = this.getFileIcon(change);
      item.tooltip = `${change.path} (${change.type})`;
      item.command = {
        command: 'vscode.open',
        title: 'Open File',
        arguments: [item.resourceUri],
      };
      return item;
    });
  }

  private getFileIcon(change: FileChange): vscode.ThemeIcon {
    switch (change.type) {
      case 'added':
      case 'untracked':
        return new vscode.ThemeIcon('diff-added', new vscode.ThemeColor('gitbutler.addedForeground'));
      case 'modified':
        return new vscode.ThemeIcon('diff-modified', new vscode.ThemeColor('gitbutler.modifiedForeground'));
      case 'deleted':
        return new vscode.ThemeIcon('diff-removed', new vscode.ThemeColor('gitbutler.deletedForeground'));
      case 'renamed':
        return new vscode.ThemeIcon('diff-renamed', new vscode.ThemeColor('gitbutler.renamedForeground'));
      default:
        return new vscode.ThemeIcon('file');
    }
  }
}

export class UnassignedTreeItem extends vscode.TreeItem {
  filePath?: string;

  constructor(
    label: string,
    collapsibleState: vscode.TreeItemCollapsibleState,
    public readonly contextValue: string,
    command?: vscode.Command
  ) {
    super(label, collapsibleState);
    if (command) {
      this.command = command;
    }
  }
}
