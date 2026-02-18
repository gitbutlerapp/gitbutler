import * as vscode from 'vscode';
import * as path from 'path';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';
import { ButApiClient } from '../services/api';
import type { FileChange, Stack } from '../types/gitbutler';

/**
 * Multi-SCM provider for GitButler.
 *
 * Creates one SourceControl instance per branch (stack) + one for unassigned changes.
 * Each branch gets its own commit input box, file list, and action buttons.
 *
 * Layout:
 *   [Unassigned Changes]        ← files not yet assigned to a branch
 *     file1.ts  [assign] [discard]
 *   [branch-name]               ← one per branch/stack
 *     Message: ___________      ← commit input box
 *     file2.rs  [unstage] [discard]
 */
export class GitButlerSCMProvider implements vscode.Disposable {
  /** SCM for unassigned changes. */
  private unassignedScm: vscode.SourceControl;
  private unassignedGroup: vscode.SourceControlResourceGroup;

  /** One SCM per stack/branch. */
  private branchScms = new Map<string, BranchSCM>();

  private disposables: vscode.Disposable[] = [];

  constructor(
    private readonly workspaceRoot: vscode.Uri,
    private readonly workspace: WorkspaceService,
    private readonly api: ButApiClient
  ) {
    // Use a rootUri that sorts before the workspace root so GitButler appears above built-in Git
    const gbRoot = workspaceRoot.with({ fragment: 'gitbutler' });

    this.unassignedScm = vscode.scm.createSourceControl(
      'gitbutler-unassigned',
      'GB: Unassigned Changes',
      gbRoot
    );
    this.unassignedScm.inputBox.visible = false;
    this.unassignedGroup = this.unassignedScm.createResourceGroup('unassigned', 'Unassigned Changes');

    this.disposables.push(
      this.workspace.onDidChangeState((state) => this.updateAll(state))
    );

    // Register content provider for diffs
    this.disposables.push(
      vscode.workspace.registerTextDocumentContentProvider(
        'gitbutler-original',
        new OriginalContentProvider(api)
      )
    );
  }

  /** Get the first branch SCM (for legacy command compat). */
  get primaryScm(): vscode.SourceControl | undefined {
    const first = this.branchScms.values().next().value;
    return first?.scm ?? this.unassignedScm;
  }

  /** Get a branch SCM by stack ID (for per-branch commit input). */
  getBranchScm(stackId: string): vscode.SourceControl | undefined {
    return this.branchScms.get(stackId)?.scm;
  }

  private updateAll(state: WorkspaceState): void {
    // -- Unassigned changes --
    this.unassignedGroup.resourceStates = state.changes.map((c) =>
      this.createResourceState(c, 'unstaged')
    );
    this.unassignedScm.count = state.changes.length;

    // -- Per-branch SCMs --
    // Collect which stacks exist and their files
    const stackMap = new Map<string, { stack: Stack; name: string; changes: FileChange[] }>();

    for (const stack of state.stacks) {
      if (!stack.id) continue;
      const name = this.getStackDisplayName(stack);
      stackMap.set(stack.id, { stack, name, changes: [] });
    }

    for (const change of state.stagedChanges) {
      if (!change.stackId) continue;
      const entry = stackMap.get(change.stackId);
      if (entry) {
        entry.changes.push(change);
      }
    }

    // Remove SCMs for stacks that no longer exist
    for (const [stackId, branchScm] of this.branchScms) {
      if (!stackMap.has(stackId)) {
        branchScm.dispose();
        this.branchScms.delete(stackId);
      }
    }

    // Create or update SCMs for each stack
    for (const [stackId, { stack, name, changes }] of stackMap) {
      let branchScm = this.branchScms.get(stackId);

      if (!branchScm) {
        // Use same gbRoot so all GitButler SCMs group together, above built-in Git
        const gbRoot = this.workspaceRoot.with({ fragment: 'gitbutler' });
        const scm = vscode.scm.createSourceControl(
          `gitbutler-branch-${stackId}`,
          `GB: ${name}`,
          gbRoot
        );
        scm.inputBox.placeholder = `Commit message for ${name} (Ctrl+Enter)`;
        scm.acceptInputCommand = {
          command: 'gitbutler.commitToStack',
          title: 'Commit',
          arguments: [stackId, name],
        };
        scm.quickDiffProvider = new GitButlerQuickDiffProvider();

        const group = scm.createResourceGroup(`branch:${stackId}`, 'Assigned Files');
        group.hideWhenEmpty = false;

        branchScm = makeBranchSCM(scm, group, stackId);
        this.branchScms.set(stackId, branchScm);
      }

      // Update
      branchScm.group.resourceStates = changes.map((c) =>
        this.createResourceState(c, 'staged')
      );
      branchScm.scm.count = changes.length;

      // Status bar command on the branch SCM
      branchScm.scm.statusBarCommands = [
        {
          command: 'gitbutler.showBranchCommits',
          title: `$(git-branch) ${name}`,
          tooltip: `GitButler: ${name}`,
        },
      ];
    }
  }

  private getStackDisplayName(stack: Stack): string {
    const names: string[] = [];
    for (const seg of stack.segments) {
      if (seg.refName) names.push(seg.refName.displayName);
    }
    return names.length > 0 ? names.join(' → ') : stack.id || 'unnamed';
  }

  private createResourceState(
    change: FileChange,
    contextValue: 'staged' | 'unstaged'
  ): vscode.SourceControlResourceState {
    const fileUri = vscode.Uri.file(path.join(this.workspaceRoot.fsPath, change.path));
    const originalUri = vscode.Uri.parse(`gitbutler-original:///${change.path}`);

    const decorations = this.getDecorations(change);

    let command: vscode.Command;
    if (change.type === 'deleted') {
      command = {
        title: 'Open Changes',
        command: 'vscode.diff',
        arguments: [originalUri, fileUri, `${change.path} (Deleted)`],
      };
    } else if (change.type === 'added' || change.type === 'untracked') {
      command = {
        title: 'Open File',
        command: 'vscode.open',
        arguments: [fileUri],
      };
    } else {
      command = {
        title: 'Open Changes',
        command: 'vscode.diff',
        arguments: [originalUri, fileUri, `${path.basename(change.path)} (Working Tree)`],
      };
    }

    return {
      resourceUri: fileUri,
      decorations,
      command,
      contextValue,
    };
  }

  private getDecorations(change: FileChange): vscode.SourceControlResourceDecorations {
    switch (change.type) {
      case 'added':
      case 'untracked':
        return {
          tooltip: change.type === 'untracked' ? 'Untracked' : 'Added',
          iconPath: new vscode.ThemeIcon('diff-added', new vscode.ThemeColor('gitbutler.addedForeground')),
          strikeThrough: false,
        };
      case 'modified':
        return {
          tooltip: 'Modified',
          iconPath: new vscode.ThemeIcon('diff-modified', new vscode.ThemeColor('gitbutler.modifiedForeground')),
        };
      case 'deleted':
        return {
          tooltip: 'Deleted',
          iconPath: new vscode.ThemeIcon('diff-removed', new vscode.ThemeColor('gitbutler.deletedForeground')),
          strikeThrough: true,
        };
      case 'renamed':
        return {
          tooltip: `Renamed${change.previousPath ? ` from ${change.previousPath}` : ''}`,
          iconPath: new vscode.ThemeIcon('diff-renamed', new vscode.ThemeColor('gitbutler.renamedForeground')),
        };
      default:
        return {
          tooltip: 'Changed',
          iconPath: new vscode.ThemeIcon('diff-modified'),
        };
    }
  }

  dispose(): void {
    this.unassignedScm.dispose();
    this.unassignedGroup.dispose();
    for (const branchScm of this.branchScms.values()) {
      branchScm.dispose();
    }
    for (const d of this.disposables) d.dispose();
  }
}

interface BranchSCM {
  scm: vscode.SourceControl;
  group: vscode.SourceControlResourceGroup;
  stackId: string;
  dispose(): void;
}

// Make BranchSCM disposable
const makeBranchSCM = (scm: vscode.SourceControl, group: vscode.SourceControlResourceGroup, stackId: string): BranchSCM => ({
  scm,
  group,
  stackId,
  dispose() {
    group.dispose();
    scm.dispose();
  },
});

class GitButlerQuickDiffProvider implements vscode.QuickDiffProvider {
  provideOriginalResource(uri: vscode.Uri): vscode.Uri | undefined {
    return vscode.Uri.parse(`gitbutler-original:///${vscode.workspace.asRelativePath(uri)}`);
  }
}

class OriginalContentProvider implements vscode.TextDocumentContentProvider {
  private _onDidChange = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this._onDidChange.event;

  constructor(private readonly api: ButApiClient) {}

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const relativePath = uri.path.startsWith('/') ? uri.path.substring(1) : uri.path;
    try {
      return await this.api.getWorkspaceFile(relativePath);
    } catch {
      return '';
    }
  }
}
