import * as vscode from 'vscode';
import * as path from 'path';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';
import { ButApiClient, pathToBytes } from '../services/api';
import type { Stack, Segment, Commit, PushStatus, FileChange } from '../types/gitbutler';

const DRAG_MIME = 'application/vnd.code.tree.gitbutlerpanel';

/**
 * Single merged tree view for GitButler — shows inside the Source Control sidebar.
 *
 * Layout:
 *   [stack-name]                  ← one per stack (expanded)
 *     [branch-name]              ← segment within stack
 *       [Assigned Files]
 *         file.ts  [unstage] [discard] [open]
 *       [Commits]
 *         fix: something   abc1234
 *   [Unassigned Changes]         ← files not assigned to any branch
 *     file.ts  [stage] [discard] [open]
 */
export class GitButlerPanelProvider
  implements vscode.TreeDataProvider<PanelItem>, vscode.TreeDragAndDropController<PanelItem>
{
  private _onDidChangeTreeData = new vscode.EventEmitter<PanelItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  readonly dropMimeTypes = [DRAG_MIME];
  readonly dragMimeTypes = [DRAG_MIME];

  private state: WorkspaceState | null = null;

  constructor(
    private readonly workspace: WorkspaceService,
    private readonly api: ButApiClient
  ) {
    workspace.onDidChangeState((state) => {
      this.state = state;
      this._onDidChangeTreeData.fire(undefined);
    });
  }

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  // ---------------------------------------------------------------------------
  // Drag & Drop
  // ---------------------------------------------------------------------------

  handleDrag(
    source: readonly PanelItem[],
    dataTransfer: vscode.DataTransfer,
    _token: vscode.CancellationToken
  ): void {
    const fileItems = source.filter(
      (item) => item.contextValue === 'file' || item.contextValue === 'unassigned-file'
    );
    if (fileItems.length === 0) return;
    dataTransfer.set(
      DRAG_MIME,
      new vscode.DataTransferItem(fileItems.map((f) => ({ path: f.filePath!, stackId: f.stackId })))
    );
  }

  async handleDrop(
    target: PanelItem | undefined,
    dataTransfer: vscode.DataTransfer,
    _token: vscode.CancellationToken
  ): Promise<void> {
    if (!target) return;

    // Find the target stack ID from whatever we dropped onto
    const resolved = this.resolveDropTarget(target);
    if (resolved === undefined) return; // undefined = invalid target, null = unassign

    const transferItem = dataTransfer.get(DRAG_MIME);
    if (!transferItem) return;

    const files = transferItem.value as Array<{ path: string; stackId: string | null }>;
    if (!files || files.length === 0) return;

    try {
      const assignments = files.map((f) => ({
        hunkHeader: null,
        pathBytes: pathToBytes(f.path),
        stackId: resolved,
      }));
      await this.api.assignHunk(assignments);
      await this.workspace.refresh();
    } catch {
      vscode.window.showErrorMessage('GitButler: Failed to move files');
    }
  }

  /** Returns stackId to assign to, null to unassign, or undefined if invalid drop target. */
  private resolveDropTarget(target: PanelItem): string | null | undefined {
    // Dropped on a branch node
    if (target.contextValue === 'branch' && target.stackId) {
      return target.stackId;
    }
    // Dropped on a stack node
    if (target.contextValue === 'stack' && target.stack?.id) {
      return target.stack.id;
    }
    // Dropped on a file that belongs to a branch
    if (target.contextValue === 'file' && target.stackId) {
      return target.stackId;
    }
    // Dropped on unassigned = unassign (stackId null)
    if (target.contextValue === 'unassigned-group' || target.contextValue === 'unassigned-file' || target.contextValue === 'unassigned-folder') {
      return null;
    }
    return undefined;
  }

  getTreeItem(element: PanelItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: PanelItem): PanelItem[] {
    if (!this.state?.isReady) {
      return [
        new PanelItem(
          'GitButler not initialized',
          vscode.TreeItemCollapsibleState.None,
          'info',
          { command: 'gitbutler.setup', title: 'Setup GitButler' }
        ),
      ];
    }

    if (!element) {
      return this.getRootItems();
    }

    switch (element.contextValue) {
      case 'stack':
        return this.getSegmentItems(element.stack!);
      case 'branch':
        return this.getBranchChildren(element);
      case 'commits-group':
        return this.getCommitItems(element.segment!);
      case 'unassigned-group':
        return this.getUnassignedItems();
      case 'unassigned-folder':
        return this.getUnassignedFolderChildren(element.folderPath!);
      default:
        return [];
    }
  }

  // ---------------------------------------------------------------------------
  // Root: stacks + unassigned group
  // ---------------------------------------------------------------------------

  private getRootItems(): PanelItem[] {
    const items: PanelItem[] = [];
    const stacks = this.state?.stacks || [];
    console.log(`[GitButler Panel] getRootItems: ${stacks.length} stacks, ${this.state?.changes.length ?? 0} unassigned`);

    // Stacks / branches
    if (stacks.length > 0) {
      for (const stack of stacks) {
        if (!stack.id) continue;
        // If stack has only one segment, show branch directly at root
        const segments = stack.segments.filter((s) => s.refName);
        if (segments.length === 1) {
          const seg = segments[0];
          const name = seg.refName?.displayName || '(unnamed)';
          const fileCount = this.getFilesForStack(stack.id).length;
          const item = new PanelItem(
            name,
            vscode.TreeItemCollapsibleState.Expanded,
            'branch'
          );
          item.segment = seg;
          item.stack = stack;
          item.stackId = stack.id;
          item.iconPath = this.getPushStatusIcon(seg.pushStatus);
          const parts: string[] = [];
          if (fileCount > 0) parts.push(`${fileCount} file${fileCount !== 1 ? 's' : ''}`);
          if (seg.commits.length > 0) parts.push(`${seg.commits.length} commit${seg.commits.length !== 1 ? 's' : ''}`);
          item.description = parts.join(' · ');
          items.push(item);
        } else {
          // Multi-segment stack
          const name = this.getStackName(stack);
          const item = new PanelItem(
            name,
            vscode.TreeItemCollapsibleState.Expanded,
            'stack'
          );
          item.stack = stack;
          item.iconPath = new vscode.ThemeIcon('layers');
          item.description = `${segments.length} branch${segments.length !== 1 ? 'es' : ''}`;
          items.push(item);
        }
      }
    }

    // Unassigned changes group
    const unassignedCount = this.state?.changes.length || 0;
    const unassignedItem = new PanelItem(
      'Unassigned Changes',
      unassignedCount > 0
        ? vscode.TreeItemCollapsibleState.Expanded
        : vscode.TreeItemCollapsibleState.Collapsed,
      'unassigned-group'
    );
    unassignedItem.iconPath = new vscode.ThemeIcon('inbox');
    unassignedItem.description = unassignedCount > 0 ? `${unassignedCount}` : '';
    items.push(unassignedItem);

    return items;
  }

  // ---------------------------------------------------------------------------
  // Stack children: segments (branches)
  // ---------------------------------------------------------------------------

  private getSegmentItems(stack: Stack): PanelItem[] {
    return stack.segments
      .filter((seg) => seg.refName)
      .map((segment) => {
        const name = segment.refName?.displayName || '(unnamed)';
        const fileCount = this.getFilesForStack(stack.id || '').length;
        const item = new PanelItem(
          name,
          vscode.TreeItemCollapsibleState.Collapsed,
          'branch'
        );
        item.segment = segment;
        item.stack = stack;
        item.stackId = stack.id || null;
        item.iconPath = this.getPushStatusIcon(segment.pushStatus);
        const parts: string[] = [];
        if (fileCount > 0) parts.push(`${fileCount} file${fileCount !== 1 ? 's' : ''}`);
        if (segment.commits.length > 0) parts.push(`${segment.commits.length} commit${segment.commits.length !== 1 ? 's' : ''}`);
        item.description = parts.join(' · ');
        return item;
      });
  }

  // ---------------------------------------------------------------------------
  // Branch children: files group + commits group
  // ---------------------------------------------------------------------------

  private getBranchChildren(element: PanelItem): PanelItem[] {
    const items: PanelItem[] = [];
    const stackId = element.stackId || '';

    // Files directly under the branch
    const files = this.getFilesForStack(stackId);
    for (const change of files) {
      const fileName = path.basename(change.path);
      const dirName = path.dirname(change.path);
      const item = new PanelItem(
        fileName,
        vscode.TreeItemCollapsibleState.None,
        'file'
      );
      item.filePath = change.path;
      item.stackId = stackId;
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
      items.push(item);
    }

    // Commits group
    if (element.segment && element.segment.commits.length > 0) {
      const commitsGroup = new PanelItem(
        'Commits',
        vscode.TreeItemCollapsibleState.Collapsed,
        'commits-group'
      );
      commitsGroup.segment = element.segment;
      commitsGroup.iconPath = new vscode.ThemeIcon('git-commit');
      commitsGroup.description = `${element.segment.commits.length}`;
      items.push(commitsGroup);
    }

    return items;
  }

  // ---------------------------------------------------------------------------
  // Commit items
  // ---------------------------------------------------------------------------

  private getCommitItems(segment: Segment): PanelItem[] {
    const items: PanelItem[] = [];

    for (const commit of segment.commits) {
      const item = new PanelItem(
        this.truncateMessage(commit.message),
        vscode.TreeItemCollapsibleState.None,
        'commit'
      );
      item.commit = commit;
      item.iconPath = this.getCommitIcon(commit);
      item.description = commit.id.substring(0, 7);
      item.tooltip = new vscode.MarkdownString(
        `**${this.escapeMarkdown(commit.message)}**\n\n` +
          `Author: ${commit.author.name} <${commit.author.email}>\n\n` +
          `SHA: \`${commit.id}\`\n\n` +
          `State: ${commit.state.type}` +
          (commit.hasConflicts ? '\n\n$(warning) **Conflicted**' : '')
      );
      items.push(item);
    }

    if (segment.commitsOnRemote.length > 0) {
      const separator = new PanelItem(
        '── Remote commits ──',
        vscode.TreeItemCollapsibleState.None,
        'separator'
      );
      separator.iconPath = new vscode.ThemeIcon('cloud');
      items.push(separator);

      for (const commit of segment.commitsOnRemote) {
        const item = new PanelItem(
          this.truncateMessage(commit.message),
          vscode.TreeItemCollapsibleState.None,
          'remote-commit'
        );
        item.iconPath = new vscode.ThemeIcon(
          'git-commit',
          new vscode.ThemeColor('descriptionForeground')
        );
        item.description = commit.id.substring(0, 7);
        items.push(item);
      }
    }

    return items;
  }

  // ---------------------------------------------------------------------------
  // Unassigned file items
  // ---------------------------------------------------------------------------

  private getUnassignedItems(): PanelItem[] {
    const changes = this.state?.changes || [];
    if (changes.length === 0) {
      const item = new PanelItem(
        'No unassigned changes',
        vscode.TreeItemCollapsibleState.None,
        'info'
      );
      item.iconPath = new vscode.ThemeIcon('check');
      return [item];
    }

    return this.buildFolderTree(changes, '', 'unassigned-file', 'unassigned-folder');
  }

  private getUnassignedFolderChildren(folderPath: string): PanelItem[] {
    const changes = this.state?.changes || [];
    const inFolder = changes.filter((c) => {
      const dir = path.dirname(c.path);
      return dir === folderPath || dir.startsWith(folderPath + '/');
    });
    return this.buildFolderTree(inFolder, folderPath, 'unassigned-file', 'unassigned-folder');
  }

  /**
   * Build a folder tree from a list of file changes.
   * Files at the current level are shown directly; subdirectories become collapsible folders.
   */
  private buildFolderTree(
    changes: FileChange[],
    prefix: string,
    fileContext: string,
    folderContext: string
  ): PanelItem[] {
    const items: PanelItem[] = [];
    const folders = new Map<string, FileChange[]>();
    const directFiles: FileChange[] = [];

    for (const change of changes) {
      // Get the path relative to the current prefix
      const relativePath = prefix ? change.path.substring(prefix.length + 1) : change.path;
      const slashIdx = relativePath.indexOf('/');

      if (slashIdx === -1) {
        // File at this level
        directFiles.push(change);
      } else {
        // File in a subdirectory
        const folderName = relativePath.substring(0, slashIdx);
        const fullFolderPath = prefix ? `${prefix}/${folderName}` : folderName;
        const arr = folders.get(fullFolderPath) || [];
        arr.push(change);
        folders.set(fullFolderPath, arr);
      }
    }

    // Sort folders first, then files
    const sortedFolders = Array.from(folders.entries()).sort(([a], [b]) => a.localeCompare(b));
    for (const [fullFolderPath, folderChanges] of sortedFolders) {
      const folderName = path.basename(fullFolderPath);
      const item = new PanelItem(
        folderName,
        vscode.TreeItemCollapsibleState.Expanded,
        folderContext
      );
      item.folderPath = fullFolderPath;
      item.iconPath = vscode.ThemeIcon.Folder;
      item.description = `${folderChanges.length}`;
      items.push(item);
    }

    for (const change of directFiles.sort((a, b) => a.path.localeCompare(b.path))) {
      const fileName = path.basename(change.path);
      const item = new PanelItem(
        fileName,
        vscode.TreeItemCollapsibleState.None,
        fileContext
      );
      item.filePath = change.path;
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
      items.push(item);
    }

    return items;
  }

  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  private getFilesForStack(stackId: string): FileChange[] {
    if (!this.state) return [];
    return this.state.stagedChanges.filter((c) => c.stackId === stackId);
  }

  private getStackName(stack: Stack): string {
    for (const segment of stack.segments) {
      if (segment.refName) return segment.refName.displayName;
    }
    return stack.id || '(unnamed stack)';
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

  private getPushStatusIcon(status: PushStatus): vscode.ThemeIcon {
    switch (status) {
      case 'nothingToPush':
        return new vscode.ThemeIcon('git-branch', new vscode.ThemeColor('charts.green'));
      case 'unpushedCommits':
        return new vscode.ThemeIcon('arrow-up', new vscode.ThemeColor('charts.yellow'));
      case 'unpushedCommitsRequiringForce':
        return new vscode.ThemeIcon('warning', new vscode.ThemeColor('charts.orange'));
      case 'completelyUnpushed':
        return new vscode.ThemeIcon('git-branch-create', new vscode.ThemeColor('charts.blue'));
      case 'integrated':
        return new vscode.ThemeIcon('check', new vscode.ThemeColor('charts.green'));
      default:
        return new vscode.ThemeIcon('git-branch');
    }
  }

  private getCommitIcon(commit: Commit): vscode.ThemeIcon {
    if (commit.hasConflicts) {
      return new vscode.ThemeIcon('warning', new vscode.ThemeColor('charts.red'));
    }
    switch (commit.state.type) {
      case 'LocalOnly':
        return new vscode.ThemeIcon('git-commit', new vscode.ThemeColor('charts.yellow'));
      case 'LocalAndRemote':
        return new vscode.ThemeIcon('git-commit', new vscode.ThemeColor('charts.green'));
      case 'Integrated':
        return new vscode.ThemeIcon('check', new vscode.ThemeColor('charts.green'));
      default:
        return new vscode.ThemeIcon('git-commit');
    }
  }

  private truncateMessage(message: string): string {
    const firstLine = message.split('\n')[0] || message;
    return firstLine.length > 60 ? firstLine.substring(0, 57) + '...' : firstLine;
  }

  private escapeMarkdown(text: string): string {
    return text.replace(/[\\`*_{}[\]()#+\-.!]/g, '\\$&');
  }
}

export class PanelItem extends vscode.TreeItem {
  stack?: Stack;
  segment?: Segment;
  commit?: Commit;
  filePath?: string;
  folderPath?: string;
  stackId?: string | null;

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
