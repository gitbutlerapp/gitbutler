import * as vscode from 'vscode';

/**
 * A minimal tree data provider that shows a single message item.
 * Used when the extension can't fully activate (no workspace, no CLI, etc.).
 */
export class EmptyTreeProvider implements vscode.TreeDataProvider<vscode.TreeItem> {
  constructor(private readonly message: string) {}

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(): vscode.TreeItem[] {
    const item = new vscode.TreeItem(this.message, vscode.TreeItemCollapsibleState.None);
    item.iconPath = new vscode.ThemeIcon('info');
    return [item];
  }
}
