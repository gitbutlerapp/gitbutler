import * as vscode from 'vscode';
import * as path from 'path';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';
import { ButApiClient, pathToBytes } from '../services/api';
import type { Stack, Segment, Commit, UpstreamCommit, PushStatus, FileChange } from '../types/gitbutler';

const FILE_TRANSFER_MIME = 'application/vnd.code.tree.gitbutlerbranches';

/**
 * TreeView showing GitButler stacks, branches (segments), commits, AND assigned files.
 * Supports drag-and-drop to move files between branches.
 */
export class BranchesTreeProvider
  implements vscode.TreeDataProvider<BranchTreeItem>, vscode.TreeDragAndDropController<BranchTreeItem>
{
  private _onDidChangeTreeData = new vscode.EventEmitter<BranchTreeItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  readonly dropMimeTypes = [FILE_TRANSFER_MIME];
  readonly dragMimeTypes = [FILE_TRANSFER_MIME];

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

  getTreeItem(element: BranchTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: BranchTreeItem): BranchTreeItem[] {
    if (!this.state?.isReady) {
      return [
        new BranchTreeItem(
          'GitButler not initialized',
          vscode.TreeItemCollapsibleState.None,
          'info',
          { command: 'gitbutler.setup', title: 'Setup GitButler' }
        ),
      ];
    }

    if (!element) {
      return this.getStackItems();
    }

    if (element.contextValue === 'stack') {
      return this.getSegmentItems(element.stack!);
    }

    if (element.contextValue === 'branch') {
      return this.getBranchChildren(element);
    }

    if (element.contextValue === 'files-group') {
      return this.getFileItems(element.stackId!);
    }

    if (element.contextValue === 'commits-group') {
      return this.getCommitItems(element.segment!);
    }

    return [];
  }

  // ---------------------------------------------------------------------------
  // Drag & Drop
  // ---------------------------------------------------------------------------

  handleDrag(
    source: readonly BranchTreeItem[],
    dataTransfer: vscode.DataTransfer,
    _token: vscode.CancellationToken
  ): void {
    // Only allow dragging file items
    const fileItems = source.filter((item) => item.contextValue === 'file');
    if (fileItems.length === 0) return;
    dataTransfer.set(
      FILE_TRANSFER_MIME,
      new vscode.DataTransferItem(fileItems.map((f) => ({ path: f.filePath!, stackId: f.stackId })))
    );
  }

  async handleDrop(
    target: BranchTreeItem | undefined,
    dataTransfer: vscode.DataTransfer,
    _token: vscode.CancellationToken
  ): Promise<void> {
    if (!target) return;

    // Find the target stack ID
    let targetStackId: string | undefined;
    if (target.contextValue === 'stack' && target.stack?.id) {
      targetStackId = target.stack.id;
    } else if (target.contextValue === 'branch' && target.segment) {
      // Find the stack that contains this segment
      targetStackId = this.findStackIdForSegment(target.segment);
    } else if (target.contextValue === 'files-group') {
      targetStackId = target.stackId ?? undefined;
    } else if (target.contextValue === 'file') {
      targetStackId = target.stackId ?? undefined;
    }
    if (!targetStackId) return;

    const transferItem = dataTransfer.get(FILE_TRANSFER_MIME);
    if (!transferItem) return;

    const files = transferItem.value as Array<{ path: string; stackId: string | null }>;
    if (!files || files.length === 0) return;

    // Build assign_hunk calls
    const { ButApiClient, pathToBytes } = await import('../services/api');
    const api = (this.workspace as any).api as InstanceType<typeof ButApiClient>;
    if (!api) return;

    try {
      const assignments = files.map((f) => ({
        hunkHeader: null,
        pathBytes: pathToBytes(f.path),
        stackId: targetStackId!,
      }));
      await api.assignHunk(assignments);
      await this.workspace.refresh();
    } catch {
      vscode.window.showErrorMessage('GitButler: Failed to move files');
    }
  }

  // ---------------------------------------------------------------------------
  // Tree structure
  // ---------------------------------------------------------------------------

  private getStackItems(): BranchTreeItem[] {
    const stacks = this.state?.stacks || [];

    if (stacks.length === 0) {
      return [
        new BranchTreeItem(
          'No stacks',
          vscode.TreeItemCollapsibleState.None,
          'info'
        ),
      ];
    }

    return stacks.map((stack) => {
      const name = this.getStackName(stack);
      const fileCount = this.getFilesForStack(stack.id || '').length;
      const item = new BranchTreeItem(
        name,
        vscode.TreeItemCollapsibleState.Expanded,
        'stack'
      );
      item.stack = stack;
      item.iconPath = new vscode.ThemeIcon('layers');
      const parts: string[] = [];
      parts.push(`${stack.segments.length} branch${stack.segments.length !== 1 ? 'es' : ''}`);
      if (fileCount > 0) {
        parts.push(`${fileCount} file${fileCount !== 1 ? 's' : ''}`);
      }
      item.description = parts.join(' · ');
      return item;
    });
  }

  private getSegmentItems(stack: Stack): BranchTreeItem[] {
    return stack.segments.map((segment) => {
      const name = segment.refName?.displayName || '(unnamed)';
      const fileCount = this.getFilesForStack(stack.id || '').length;
      const item = new BranchTreeItem(
        name,
        vscode.TreeItemCollapsibleState.Collapsed,
        'branch'
      );
      item.segment = segment;
      item.stackId = stack.id || null;
      item.iconPath = this.getPushStatusIcon(segment.pushStatus);

      const parts: string[] = [];
      if (fileCount > 0) {
        parts.push(`${fileCount} file${fileCount !== 1 ? 's' : ''}`);
      }
      if (segment.commits.length > 0) {
        parts.push(`${segment.commits.length} commit${segment.commits.length !== 1 ? 's' : ''}`);
      }
      if (segment.isEntrypoint) {
        parts.push('current');
      }
      item.description = parts.join(' · ');

      const tooltipParts = [`Branch: ${name}`];
      if (segment.remoteTrackingRefName) {
        tooltipParts.push(`Remote: ${segment.remoteTrackingRefName.displayName}`);
      }
      tooltipParts.push(`Push status: ${segment.pushStatus}`);
      item.tooltip = tooltipParts.join('\n');

      return item;
    });
  }

  private getBranchChildren(element: BranchTreeItem): BranchTreeItem[] {
    const items: BranchTreeItem[] = [];
    const stackId = element.stackId || '';
    const files = this.getFilesForStack(stackId);

    // Files group (always show even if empty, to accept drops)
    const filesGroup = new BranchTreeItem(
      `Assigned Files`,
      files.length > 0 ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.Collapsed,
      'files-group'
    );
    filesGroup.stackId = stackId;
    filesGroup.segment = element.segment;
    filesGroup.iconPath = new vscode.ThemeIcon('files');
    filesGroup.description = files.length > 0 ? `${files.length}` : 'none';
    items.push(filesGroup);

    // Commits group
    if (element.segment && element.segment.commits.length > 0) {
      const commitsGroup = new BranchTreeItem(
        `Commits`,
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

  private getFileItems(stackId: string): BranchTreeItem[] {
    const files = this.getFilesForStack(stackId);
    return files.map((change) => {
      const fileName = path.basename(change.path);
      const dirName = path.dirname(change.path);
      const item = new BranchTreeItem(
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
      return item;
    });
  }

  private getCommitItems(segment: Segment): BranchTreeItem[] {
    const items: BranchTreeItem[] = [];

    for (const commit of segment.commits) {
      const item = new BranchTreeItem(
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
      const separator = new BranchTreeItem(
        `── Remote commits ──`,
        vscode.TreeItemCollapsibleState.None,
        'separator'
      );
      separator.iconPath = new vscode.ThemeIcon('cloud');
      items.push(separator);

      for (const commit of segment.commitsOnRemote) {
        const item = new BranchTreeItem(
          this.truncateMessage(commit.message),
          vscode.TreeItemCollapsibleState.None,
          'remote-commit'
        );
        item.iconPath = new vscode.ThemeIcon('git-commit', new vscode.ThemeColor('descriptionForeground'));
        item.description = commit.id.substring(0, 7);
        items.push(item);
      }
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

  private findStackIdForSegment(segment: Segment): string | undefined {
    if (!this.state) return undefined;
    for (const stack of this.state.stacks) {
      for (const seg of stack.segments) {
        if (seg.refName?.displayName === segment.refName?.displayName) {
          return stack.id || undefined;
        }
      }
    }
    return undefined;
  }

  private getStackName(stack: Stack): string {
    for (const segment of stack.segments) {
      if (segment.refName) {
        return segment.refName.displayName;
      }
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

export class BranchTreeItem extends vscode.TreeItem {
  stack?: Stack;
  segment?: Segment;
  commit?: Commit;
  filePath?: string;
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
