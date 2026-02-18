import * as vscode from 'vscode';
import * as path from 'path';
import { ButApiClient, ApiError, pathToBytes, type DiffSpec } from './services/api';
import { WorkspaceService } from './services/workspace';
import type { FileChange, TreeChange, HunkAssignment } from './types/gitbutler';

/**
 * Register all GitButler commands.
 */
export function registerCommands(
  context: vscode.ExtensionContext,
  api: ButApiClient,
  workspace: WorkspaceService
): void {
  // -----------------------------------------------------------------------
  // Refresh
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.refresh', async () => {
      await vscode.window.withProgress(
        { location: vscode.ProgressLocation.SourceControl },
        () => workspace.refresh()
      );
    })
  );

  // -----------------------------------------------------------------------
  // Commit (generic — picks a branch if needed)
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.commit', async () => {
      const { stagedChanges, stacks } = workspace.state;
      if (stagedChanges.length === 0) {
        vscode.window.showWarningMessage('GitButler: No changes assigned to any branch');
        return;
      }

      const byStack = new Map<string, FileChange[]>();
      for (const change of stagedChanges) {
        if (change.stackId) {
          const arr = byStack.get(change.stackId) || [];
          arr.push(change);
          byStack.set(change.stackId, arr);
        }
      }

      if (byStack.size === 0) {
        vscode.window.showWarningMessage('GitButler: Changes are not assigned to a branch');
        return;
      }

      let targetStackId: string;
      if (byStack.size === 1) {
        targetStackId = byStack.keys().next().value!;
      } else {
        const picks = Array.from(byStack.keys()).map((sid) => {
          const name = getStackBranchName(stacks, sid);
          return { label: name || sid, stackId: sid };
        });
        const pick = await vscode.window.showQuickPick(picks, {
          placeHolder: 'Which branch to commit?',
        });
        if (!pick) return;
        targetStackId = pick.stackId;
      }

      const message =
        (await vscode.window.showInputBox({
          prompt: 'Enter commit message',
          placeHolder: 'Commit message',
        })) || '';
      if (!message) return;

      await commitToStack(api, workspace, targetStackId, message);
    })
  );

  // -----------------------------------------------------------------------
  // Commit to specific stack (called from per-branch SCM accept)
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand(
      'gitbutler.commitToStack',
      async (stackId: string, branchName: string) => {
        const message =
          (await vscode.window.showInputBox({
            prompt: `Commit message for ${branchName}`,
            placeHolder: 'Commit message',
          })) || '';
        if (!message) return;

        await commitToStack(api, workspace, stackId, message);
      }
    )
  );

  // -----------------------------------------------------------------------
  // Generate commit message (AI)
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.generateCommitMessage', async () => {
      const { stagedChanges, stacks } = workspace.state;
      if (stagedChanges.length === 0) {
        vscode.window.showWarningMessage('GitButler: No assigned changes to summarize');
        return;
      }

      // Pick which branch if multiple
      const byStack = new Map<string, FileChange[]>();
      for (const c of stagedChanges) {
        if (c.stackId) {
          const arr = byStack.get(c.stackId) || [];
          arr.push(c);
          byStack.set(c.stackId, arr);
        }
      }

      let targetStackId: string;
      if (byStack.size === 1) {
        targetStackId = byStack.keys().next().value!;
      } else if (byStack.size > 1) {
        const picks = Array.from(byStack.keys()).map((sid) => ({
          label: getStackBranchName(stacks, sid) || sid,
          stackId: sid,
        }));
        const pick = await vscode.window.showQuickPick(picks, {
          placeHolder: 'Generate message for which branch?',
        });
        if (!pick) return;
        targetStackId = pick.stackId;
      } else {
        return;
      }

      const changes = byStack.get(targetStackId) || [];
      const branchName = getStackBranchName(stacks, targetStackId);

      try {
        const message = await vscode.window.withProgress(
          { location: vscode.ProgressLocation.Notification, title: 'Generating commit message...' },
          () => generateMessage(changes, branchName)
        );
        if (message) {
          // Show the generated message in a quick input for review
          const confirmed = await vscode.window.showInputBox({
            prompt: 'Generated commit message (edit or press Enter to use)',
            value: message,
          });
          if (confirmed) {
            await commitToStack(api, workspace, targetStackId, confirmed);
          }
        }
      } catch (err) {
        showError('Generate message failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Commit to specific branch (via QuickPick)
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.commitToBranch', async () => {
      const branches = getBranchPicks(workspace);
      if (branches.length === 0) {
        vscode.window.showWarningMessage('No branches available');
        return;
      }
      const pick = await vscode.window.showQuickPick(branches, {
        placeHolder: 'Select branch to commit to',
      });
      if (!pick) return;

      const message =
        (await vscode.window.showInputBox({
          prompt: 'Enter commit message',
          placeHolder: 'Commit message',
        })) || '';
      if (!message) return;

      await commitToStack(api, workspace, pick.stackId, message);
    })
  );

  // -----------------------------------------------------------------------
  // Stage / Unstage individual files
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.stageFile', async (...args: unknown[]) => {
      const fileItems = extractFileItems(args);
      if (fileItems.length === 0) return;

      // Need to pick a stack to assign to
      const stacks = workspace.state.stacks;
      let targetStackId: string | undefined;

      if (stacks.length === 1 && stacks[0]?.id) {
        targetStackId = stacks[0].id;
      } else if (stacks.length > 1) {
        const picks = getBranchPicks(workspace);
        const pick = await vscode.window.showQuickPick(picks, {
          placeHolder: 'Stage to which branch?',
        });
        if (!pick) return;
        targetStackId = pick.stackId;
      }

      if (!targetStackId) {
        vscode.window.showWarningMessage('No stacks available to stage to');
        return;
      }

      try {
        const assignments = fileItems.map((fi) => {
          const change = findChangeByPath(workspace, fi.relativePath);
          return {
            hunkHeader: change ? getFirstHunkHeader(change, workspace) : null,
            pathBytes: pathToBytes(fi.relativePath),
            stackId: targetStackId!,
          };
        });
        await api.assignHunk(assignments);
        await workspace.refresh();
      } catch (err) {
        showError('Stage failed', err);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.unstageFile', async (...args: unknown[]) => {
      const fileItems = extractFileItems(args);
      if (fileItems.length === 0) return;

      try {
        const assignments = fileItems.map((fi) => {
          const change = findChangeByPath(workspace, fi.relativePath);
          return {
            hunkHeader: change ? getFirstHunkHeader(change, workspace) : null,
            pathBytes: pathToBytes(fi.relativePath),
            stackId: null, // null = unassign
          };
        });
        await api.assignHunk(assignments);
        await workspace.refresh();
      } catch (err) {
        showError('Unstage failed', err);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.discardFile', async (...args: unknown[]) => {
      const fileItems = extractFileItems(args);
      if (fileItems.length === 0) return;

      const confirm = await vscode.window.showWarningMessage(
        `Discard changes to ${fileItems.length} file${fileItems.length !== 1 ? 's' : ''}? This cannot be undone.`,
        { modal: true },
        'Discard'
      );
      if (confirm !== 'Discard') return;

      try {
        const diffSpecs: DiffSpec[] = fileItems.map((fi) => ({
          previousPathBytes: null,
          pathBytes: pathToBytes(fi.relativePath),
          hunkHeaders: [], // empty = discard entire file
        }));
        await api.discardChanges(diffSpecs);
        await workspace.refresh();
      } catch (err) {
        showError('Discard failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Stage All / Unstage All
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.stageAll', async () => {
      const { changes, stacks } = workspace.state;
      if (changes.length === 0) return;

      let targetStackId: string | undefined;
      if (stacks.length === 1 && stacks[0]?.id) {
        targetStackId = stacks[0].id;
      } else if (stacks.length > 1) {
        const picks = getBranchPicks(workspace);
        const pick = await vscode.window.showQuickPick(picks, {
          placeHolder: 'Stage all to which branch?',
        });
        if (!pick) return;
        targetStackId = pick.stackId;
      }
      if (!targetStackId) return;

      try {
        const assignments = changes.map((c) => ({
          hunkHeader: getFirstHunkHeader(c, workspace),
          pathBytes: pathToBytes(c.path),
          stackId: targetStackId!,
        }));
        await api.assignHunk(assignments);
        await workspace.refresh();
      } catch (err) {
        showError('Stage all failed', err);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.unstageAll', async () => {
      const { stagedChanges } = workspace.state;
      if (stagedChanges.length === 0) return;

      try {
        const assignments = stagedChanges.map((c) => ({
          hunkHeader: getFirstHunkHeader(c, workspace),
          pathBytes: pathToBytes(c.path),
          stackId: null,
        }));
        await api.assignHunk(assignments);
        await workspace.refresh();
      } catch (err) {
        showError('Unstage all failed', err);
      }
    })
  );

  // Legacy aliases
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.stage', () =>
      vscode.commands.executeCommand('gitbutler.stageAll')
    )
  );
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.unstage', () =>
      vscode.commands.executeCommand('gitbutler.unstageAll')
    )
  );
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.discard', async () => {
      const { changes } = workspace.state;
      if (changes.length === 0) return;
      const confirm = await vscode.window.showWarningMessage(
        `Discard all ${changes.length} unstaged changes? This cannot be undone.`,
        { modal: true },
        'Discard All'
      );
      if (confirm !== 'Discard All') return;
      try {
        const diffSpecs = changes.map((c) => treeChangeToDiffSpec(c.treeChange));
        await api.discardChanges(diffSpecs);
        await workspace.refresh();
      } catch (err) {
        showError('Discard failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Open file / diff
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.openFile', async (...args: unknown[]) => {
      for (const fi of extractFileItems(args)) {
        const uri = vscode.Uri.file(
          path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '', fi.relativePath)
        );
        await vscode.window.showTextDocument(uri);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.openDiff', async (...args: unknown[]) => {
      for (const fi of extractFileItems(args)) {
        const fileUri = vscode.Uri.file(
          path.join(vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '', fi.relativePath)
        );
        const originalUri = vscode.Uri.parse(`gitbutler-original:///${fi.relativePath}`);
        await vscode.commands.executeCommand(
          'vscode.diff',
          originalUri,
          fileUri,
          `${path.basename(fi.relativePath)} (Working Tree)`
        );
      }
    })
  );

  // -----------------------------------------------------------------------
  // Branch management
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.createBranch', async () => {
      const stacks = workspace.state.stacks;
      const hasBranches = stacks.length > 0;

      // Ask: independent or stacked?
      let kind: 'independent' | 'stacked' = 'independent';
      if (hasBranches) {
        const choices = [
          { label: 'Independent Branch', description: 'New parallel stack — changes are independent', value: 'independent' as const },
          { label: 'Stacked Branch', description: 'Child branch on top of an existing branch', value: 'stacked' as const },
        ];
        const pick = await vscode.window.showQuickPick(choices, {
          placeHolder: 'What kind of branch?',
        });
        if (!pick) return;
        kind = pick.value;
      }

      const name = await vscode.window.showInputBox({
        prompt: kind === 'stacked' ? 'Enter stacked branch name' : 'Enter new branch name',
        placeHolder: 'feature/my-branch',
        validateInput: (v) => (!v.trim() ? 'Name required' : /\s/.test(v) ? 'No spaces' : null),
      });
      if (!name) return;

      try {
        if (kind === 'stacked') {
          // Pick which branch to stack on
          const picks = getBranchPicks(workspace);
          const parentPick = await vscode.window.showQuickPick(picks, {
            placeHolder: 'Stack on top of which branch?',
          });
          if (!parentPick) return;

          await api.createReference({
            newName: name,
            anchor: {
              type: 'AtReference',
              subject: {
                shortName: parentPick.label,
                position: 'Above',
              },
            },
          });
        } else {
          await api.createReference({ newName: name, anchor: null });
        }
        await workspace.refresh();
        vscode.window.showInformationMessage(`GitButler: Created branch "${name}"`);
      } catch (err) {
        showError('Create branch failed', err);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.deleteBranch', async (item?: { segment?: { refName?: { displayName: string } } }) => {
      let branchName: string | undefined;
      let stackId: string | undefined;
      if (item?.segment?.refName) {
        branchName = item.segment.refName.displayName;
        // Find the stack ID for this segment
        for (const stack of workspace.state.stacks) {
          for (const seg of stack.segments) {
            if (seg.refName?.displayName === branchName && stack.id) {
              stackId = stack.id;
              break;
            }
          }
          if (stackId) break;
        }
      } else {
        const picks = getBranchPicks(workspace);
        const pick = await vscode.window.showQuickPick(picks, {
          placeHolder: 'Select branch to delete',
        });
        if (!pick) return;
        branchName = pick.label;
        stackId = pick.stackId;
      }
      if (!branchName) return;
      const confirm = await vscode.window.showWarningMessage(
        `Delete branch "${branchName}"?`,
        { modal: true },
        'Delete'
      );
      if (confirm !== 'Delete') return;
      try {
        if (stackId) {
          await api.removeBranch(stackId, branchName);
        } else {
          // Fallback: try with full ref name
          await api.deleteLocalBranch(`refs/heads/${branchName}`);
        }
        await workspace.refresh();
        vscode.window.showInformationMessage(`GitButler: Deleted "${branchName}"`);
      } catch (err) {
        showError('Delete failed', err);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.showBranchCommits', () =>
      vscode.commands.executeCommand('gitbutlerBranches.focus')
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.pushBranch', async (item?: { segment?: { refName?: { displayName: string } } }) => {
      let stackId: string | undefined;
      let branchName: string | undefined;
      if (item?.segment?.refName) {
        branchName = item.segment.refName.displayName;
        // Find the stack ID for this segment
        for (const stack of workspace.state.stacks) {
          for (const seg of stack.segments) {
            if (seg.refName?.displayName === branchName && stack.id) {
              stackId = stack.id;
              break;
            }
          }
          if (stackId) break;
        }
      }
      if (!stackId || !branchName) {
        const picks = getBranchPicks(workspace);
        const pick = await vscode.window.showQuickPick(picks, {
          placeHolder: 'Push which branch?',
        });
        if (!pick) return;
        stackId = pick.stackId;
        branchName = pick.label;
      }
      try {
        await vscode.window.withProgress(
          { location: vscode.ProgressLocation.SourceControl, title: `Pushing ${branchName}...` },
          () => api.pushStack({ stackId: stackId!, branch: branchName! })
        );
        await workspace.refresh();
        vscode.window.showInformationMessage(`GitButler: Pushed ${branchName}`);
      } catch (err) {
        showError('Push failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Move to Branch (reassign files between branches)
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.moveToBranch', async (...args: unknown[]) => {
      const fileItems = extractFileItems(args);
      if (fileItems.length === 0) return;

      const branches = getBranchPicks(workspace);
      if (branches.length === 0) {
        vscode.window.showWarningMessage('No branches available');
        return;
      }

      // Determine the current stack so we can exclude it from the pick list
      const currentChange = findChangeByPath(workspace, fileItems[0].relativePath);
      const currentStackId = currentChange?.stackId || fileItems[0].stackId;
      const filteredBranches = currentStackId
        ? branches.filter((b) => b.stackId !== currentStackId)
        : branches;

      if (filteredBranches.length === 0) {
        vscode.window.showWarningMessage('No other branches to move to');
        return;
      }

      const pick = await vscode.window.showQuickPick(filteredBranches, {
        placeHolder: 'Move file(s) to which branch?',
      });
      if (!pick) return;

      try {
        const assignments = fileItems.map((fi) => {
          const change = findChangeByPath(workspace, fi.relativePath);
          return {
            hunkHeader: change ? getFirstHunkHeader(change, workspace) : null,
            pathBytes: pathToBytes(fi.relativePath),
            stackId: pick.stackId,
          };
        });
        await api.assignHunk(assignments);
        await workspace.refresh();
        const count = fileItems.length;
        vscode.window.showInformationMessage(
          `GitButler: Moved ${count} file${count !== 1 ? 's' : ''} to ${pick.label}`
        );
      } catch (err) {
        showError('Move to branch failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Sync
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.push', async () => {
      // Push all stacks that have unpushed commits
      const { stacks } = workspace.state;
      try {
        await vscode.window.withProgress(
          { location: vscode.ProgressLocation.SourceControl, title: 'Pushing...' },
          async () => {
            for (const stack of stacks) {
              if (!stack.id) continue;
              for (const seg of stack.segments) {
                if (
                  seg.pushStatus !== 'nothingToPush' &&
                  seg.pushStatus !== 'integrated' &&
                  seg.refName
                ) {
                  await api.pushStack({
                    stackId: stack.id,
                    branch: seg.refName.displayName,
                  });
                }
              }
            }
          }
        );
        await workspace.refresh();
        vscode.window.showInformationMessage('GitButler: Push complete');
      } catch (err) {
        showError('Push failed', err);
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.pull', async () => {
      try {
        await vscode.window.withProgress(
          { location: vscode.ProgressLocation.SourceControl, title: 'Fetching...' },
          () => api.fetchFromRemotes()
        );
        await workspace.refresh();
        vscode.window.showInformationMessage('GitButler: Fetch complete');
      } catch (err) {
        showError('Fetch failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Undo
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.undo', async () => {
      const confirm = await vscode.window.showWarningMessage(
        'Undo the last GitButler operation?',
        { modal: true },
        'Undo'
      );
      if (confirm !== 'Undo') return;
      try {
        const snapshots = await api.listSnapshots();
        if (snapshots.length >= 2) {
          const prev = snapshots[1] as { id?: string; sha?: string };
          const sha = prev.sha || prev.id;
          if (sha) {
            await api.restoreSnapshot(sha);
            await workspace.refresh();
            vscode.window.showInformationMessage('GitButler: Undo complete');
            return;
          }
        }
        vscode.window.showWarningMessage('GitButler: No previous snapshot to undo to');
      } catch (err) {
        showError('Undo failed', err);
      }
    })
  );

  // -----------------------------------------------------------------------
  // Setup & GUI
  // -----------------------------------------------------------------------
  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.setup', async () => {
      vscode.window.showInformationMessage(
        'GitButler: The server manages project setup automatically.'
      );
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('gitbutler.openInGUI', async () => {
      try {
        await api.invoke('open_url', {
          url: 'gitbutler://open',
        });
      } catch {
        vscode.window.showWarningMessage(
          'GitButler: Could not open GUI. Is it installed?'
        );
      }
    })
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Item from tree view or SCM resource state — we just need the file path. */
interface FileItem {
  relativePath: string;
  stackId?: string | null;
}

/**
 * Extract file items from command arguments.
 * Handles both PanelItem (tree view) and SourceControlResourceState (SCM).
 */
function extractFileItems(args: unknown[]): FileItem[] {
  const items: FileItem[] = [];
  for (const arg of args) {
    if (Array.isArray(arg)) {
      for (const item of arg) {
        const fi = toFileItem(item);
        if (fi) items.push(fi);
      }
    } else {
      const fi = toFileItem(arg);
      if (fi) items.push(fi);
    }
  }
  return items;
}

function toFileItem(obj: unknown): FileItem | null {
  if (typeof obj !== 'object' || obj === null) return null;

  // Tree view PanelItem: has filePath
  if ('filePath' in obj && typeof (obj as any).filePath === 'string') {
    return {
      relativePath: (obj as any).filePath,
      stackId: (obj as any).stackId ?? null,
    };
  }

  // SCM SourceControlResourceState: has resourceUri
  if ('resourceUri' in obj && (obj as any).resourceUri instanceof vscode.Uri) {
    return {
      relativePath: vscode.workspace.asRelativePath((obj as any).resourceUri),
      stackId: null,
    };
  }

  return null;
}

interface BranchPick {
  label: string;
  stackId: string;
}

function getBranchPicks(workspace: WorkspaceService): BranchPick[] {
  const picks: BranchPick[] = [];
  for (const stack of workspace.state.stacks) {
    if (!stack.id) continue;
    for (const seg of stack.segments) {
      if (seg.refName) {
        picks.push({ label: seg.refName.displayName, stackId: stack.id });
      }
    }
  }
  return picks;
}

function getStackBranchName(stacks: import('./types/gitbutler').Stack[], stackId: string): string {
  for (const stack of stacks) {
    if (stack.id === stackId) {
      for (const seg of stack.segments) {
        if (seg.refName) return seg.refName.displayName;
      }
    }
  }
  return '';
}

function findChangeByPath(workspace: WorkspaceService, relativePath: string): FileChange | undefined {
  const normalizedPath = relativePath.replace(/\\/g, '/');
  return (
    workspace.state.changes.find((c) => c.path === normalizedPath) ||
    workspace.state.stagedChanges.find((c) => c.path === normalizedPath)
  );
}

function getFirstHunkHeader(
  change: FileChange,
  workspace: WorkspaceService
): import('./types/gitbutler').HunkHeader | null {
  // Find the hunk assignment for this file
  const assignment = workspace.state.rawAssignments.find((a) => a.path === change.path);
  return assignment?.hunkHeader || null;
}

function treeChangeToDiffSpec(treeChange: TreeChange): DiffSpec {
  const pathBytes = treeChange.pathBytes;
  let previousPathBytes: number[] | null = null;
  if (treeChange.status.type === 'Rename') {
    previousPathBytes = treeChange.status.subject.previousPathBytes;
  }
  return {
    previousPathBytes,
    pathBytes,
    hunkHeaders: [], // empty = take all hunks
  };
}

function showError(prefix: string, err: unknown): void {
  const message =
    err instanceof ApiError
      ? `${prefix}: ${err.detail}`
      : err instanceof Error
        ? `${prefix}: ${err.message}`
        : `${prefix}`;
  vscode.window.showErrorMessage(message);
}

function truncate(str: string, maxLen: number): string {
  const firstLine = str.split('\n')[0] || str;
  return firstLine.length > maxLen ? firstLine.substring(0, maxLen - 3) + '...' : firstLine;
}

// ---------------------------------------------------------------------------
// Commit helper
// ---------------------------------------------------------------------------
async function commitToStack(
  api: ButApiClient,
  workspace: WorkspaceService,
  stackId: string,
  message: string
): Promise<void> {
  const { stagedChanges, stacks } = workspace.state;
  const changesForStack = stagedChanges.filter((c) => c.stackId === stackId);
  if (changesForStack.length === 0) {
    vscode.window.showWarningMessage('GitButler: No changes assigned to this branch');
    return;
  }

  const branchName = getStackBranchName(stacks, stackId);
  const worktreeChanges: DiffSpec[] = changesForStack.map((c) =>
    treeChangeToDiffSpec(c.treeChange)
  );

  try {
    await vscode.window.withProgress(
      { location: vscode.ProgressLocation.SourceControl, title: 'Committing...' },
      () =>
        api.createCommit({
          stackId,
          message,
          stackBranchName: branchName,
          worktreeChanges,
        })
    );
    await workspace.refresh();
    vscode.window.showInformationMessage(`GitButler: Committed "${truncate(message, 50)}"`);
  } catch (err) {
    showError('Commit failed', err);
  }
}

// ---------------------------------------------------------------------------
// AI commit message generation
// ---------------------------------------------------------------------------
export async function generateMessage(
  changes: FileChange[],
  branchName: string
): Promise<string | undefined> {
  // Build a summary of the changes for the prompt
  const fileSummary = changes
    .map((c) => `${c.type.toUpperCase()}: ${c.path}`)
    .join('\n');

  const prompt = `Generate a concise, conventional commit message (one line, max 72 chars) for these changes on branch "${branchName}":

${fileSummary}

Rules:
- Use conventional commit format: type(scope): description
- Types: feat, fix, refactor, chore, docs, style, test, build, ci
- Be specific about what changed
- No quotes around the message
- Just output the commit message, nothing else`;

  // Try VS Code Language Model API (Copilot)
  try {
    const models = await vscode.lm.selectChatModels({ family: 'gpt-4o' });
    const model = models[0];
    if (!model) {
      // Fallback: try any available model
      const allModels = await vscode.lm.selectChatModels();
      if (allModels.length === 0) {
        vscode.window.showWarningMessage(
          'GitButler: No AI model available. Install GitHub Copilot for commit message generation.'
        );
        return undefined;
      }
      return await chatWithModel(allModels[0], prompt);
    }
    return await chatWithModel(model, prompt);
  } catch {
    // LM API not available — fallback to simple heuristic
    return generateSimpleMessage(changes, branchName);
  }
}

async function chatWithModel(
  model: vscode.LanguageModelChat,
  prompt: string
): Promise<string | undefined> {
  const messages = [vscode.LanguageModelChatMessage.User(prompt)];
  const response = await model.sendRequest(messages);

  let result = '';
  for await (const chunk of response.text) {
    result += chunk;
  }
  return result.trim().replace(/^["']|["']$/g, '');
}

function generateSimpleMessage(changes: FileChange[], branchName: string): string {
  const types = new Set(changes.map((c) => c.type));
  const dirs = new Set(changes.map((c) => {
    const parts = c.path.split('/');
    return parts.length > 1 ? parts[0] : '';
  }).filter(Boolean));

  const scope = dirs.size === 1 ? `(${[...dirs][0]})` : '';
  const count = changes.length;

  if (types.has('added') && types.size === 1) {
    return `feat${scope}: add ${count} new file${count !== 1 ? 's' : ''}`;
  }
  if (types.has('deleted') && types.size === 1) {
    return `chore${scope}: remove ${count} file${count !== 1 ? 's' : ''}`;
  }
  return `chore${scope}: update ${count} file${count !== 1 ? 's' : ''}`;
}
