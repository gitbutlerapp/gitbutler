import * as vscode from 'vscode';
import { WorkspaceService, type WorkspaceState } from '../services/workspace';

/**
 * Manages status bar items showing GitButler state.
 */
export class StatusBarService implements vscode.Disposable {
  private branchItem: vscode.StatusBarItem;
  private changesItem: vscode.StatusBarItem;
  private syncItem: vscode.StatusBarItem;

  constructor(private readonly workspace: WorkspaceService) {
    // Branch name (left side, high priority)
    this.branchItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100
    );
    this.branchItem.command = 'gitbutler.showBranchCommits';
    this.branchItem.name = 'GitButler Branch';

    // Changes count
    this.changesItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      99
    );
    this.changesItem.command = 'workbench.view.scm';
    this.changesItem.name = 'GitButler Changes';

    // Sync status
    this.syncItem = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      98
    );
    this.syncItem.name = 'GitButler Sync';

    workspace.onDidChangeState((state) => this.update(state));
  }

  private update(state: WorkspaceState): void {
    if (!state.isReady) {
      this.branchItem.hide();
      this.changesItem.hide();
      this.syncItem.hide();
      return;
    }

    // Branch item
    const branchName = this.getBranchName(state);
    if (branchName) {
      this.branchItem.text = `$(git-branch) ${branchName}`;
      this.branchItem.tooltip = `GitButler: ${branchName}\n\nClick to view commits`;
      this.branchItem.show();
    }

    // Changes count
    const totalChanges = state.changes.length + state.stagedChanges.length;
    if (totalChanges > 0) {
      const parts: string[] = [];
      if (state.stagedChanges.length > 0) {
        parts.push(`${state.stagedChanges.length} staged`);
      }
      if (state.changes.length > 0) {
        parts.push(`${state.changes.length} changed`);
      }
      this.changesItem.text = `$(diff) ${totalChanges}`;
      this.changesItem.tooltip = `GitButler: ${parts.join(', ')}\n\nClick to open Source Control`;
      this.changesItem.show();
    } else {
      this.changesItem.hide();
    }

    // Sync status
    this.updateSyncItem(state);
  }

  private updateSyncItem(state: WorkspaceState): void {
    const target = state.target;
    if (!target) {
      this.syncItem.hide();
      return;
    }

    // Check if any stack has unpushed commits
    let hasUnpushed = false;
    for (const stack of state.stacks) {
      for (const segment of stack.segments) {
        if (
          segment.pushStatus === 'unpushedCommits' ||
          segment.pushStatus === 'completelyUnpushed' ||
          segment.pushStatus === 'unpushedCommitsRequiringForce'
        ) {
          hasUnpushed = true;
          break;
        }
      }
      if (hasUnpushed) break;
    }

    const parts: string[] = [];
    if (target.commitsAhead > 0) {
      parts.push(`$(cloud-download) ${target.commitsAhead}`);
    }
    if (hasUnpushed) {
      parts.push(`$(cloud-upload)`);
    }

    if (parts.length > 0) {
      this.syncItem.text = parts.join(' ');
      this.syncItem.tooltip = [
        target.commitsAhead > 0
          ? `${target.commitsAhead} commit${target.commitsAhead !== 1 ? 's' : ''} to pull`
          : null,
        hasUnpushed ? 'Unpushed commits' : null,
      ]
        .filter(Boolean)
        .join('\n');
      this.syncItem.command = target.commitsAhead > 0 ? 'gitbutler.pull' : 'gitbutler.push';
      this.syncItem.show();
    } else {
      this.syncItem.hide();
    }
  }

  private getBranchName(state: WorkspaceState): string | null {
    if (state.refInfo?.workspaceRef) {
      return state.refInfo.workspaceRef.displayName;
    }
    if (state.stacks.length > 0) {
      const seg = state.stacks[0]?.segments?.[0];
      if (seg?.refName) {
        return seg.refName.displayName;
      }
    }
    return null;
  }

  show(): void {
    this.branchItem.show();
  }

  dispose(): void {
    this.branchItem.dispose();
    this.changesItem.dispose();
    this.syncItem.dispose();
  }
}
