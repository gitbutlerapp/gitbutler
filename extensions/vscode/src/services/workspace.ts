import * as vscode from 'vscode';
import { ButApiClient, type WorktreeChangesResponse } from './api';
import { EventService } from './events';
import type {
  FileChange,
  TreeChange,
  HunkAssignment,
  Stack,
  Target,
  RefInfo,
} from '../types/gitbutler';
import { treeStatusToChangeType } from '../types/gitbutler';

/**
 * Manages the workspace state.
 * Uses the but-server HTTP API for data and WebSocket events for reactivity.
 */
export class WorkspaceService implements vscode.Disposable {
  private _onDidChangeState = new vscode.EventEmitter<WorkspaceState>();
  readonly onDidChangeState = this._onDidChangeState.event;

  private _state: WorkspaceState = {
    changes: [],
    stagedChanges: [],
    stacks: [],
    target: null,
    refInfo: null,
    isReady: false,
    rawAssignments: [],
    rawChanges: [],
  };

  private refreshDebounce: ReturnType<typeof setTimeout> | undefined;
  private isRefreshing = false;
  private disposables: vscode.Disposable[] = [];

  constructor(
    private readonly api: ButApiClient,
    private readonly events: EventService | null
  ) {
    // Subscribe to WebSocket events for real-time updates
    if (events) {
      this.disposables.push(
        events.onWorktreeChanged(() => this.debouncedRefresh()),
        events.onHeadChanged(() => this.debouncedRefresh()),
        events.onGitActivity(() => this.debouncedRefresh())
      );
    }
  }

  get state(): WorkspaceState {
    return this._state;
  }

  async initialize(): Promise<void> {
    // If no WebSocket events, fall back to polling
    if (!this.events) {
      const config = vscode.workspace.getConfiguration('gitbutler');
      const interval = config.get('refreshInterval', 3000);
      setInterval(() => this.refresh(), interval);
    }

    // Initial refresh
    await this.refresh();
  }

  private debouncedRefresh(): void {
    if (this.refreshDebounce) {
      clearTimeout(this.refreshDebounce);
    }
    this.refreshDebounce = setTimeout(() => this.refresh(), 300);
  }

  /**
   * Refresh the workspace state from the API.
   */
  async refresh(): Promise<void> {
    if (this.isRefreshing) return;
    this.isRefreshing = true;

    try {
      // Fetch worktree changes and head info in parallel
      const [worktreeResult, headInfoResult] = await Promise.allSettled([
        this.api.changesInWorktree(),
        this.api.headInfo(),
      ]);

      const worktreeChanges =
        worktreeResult.status === 'fulfilled' ? worktreeResult.value : null;
      const headInfo =
        headInfoResult.status === 'fulfilled' ? headInfoResult.value : null;



      const newState = this.buildState(worktreeChanges, headInfo);
      this._state = newState;
      this._onDidChangeState.fire(newState);
    } catch {
      this._state = { ...this._state, isReady: false };
      this._onDidChangeState.fire(this._state);
    } finally {
      this.isRefreshing = false;
    }
  }

  private buildState(
    worktree: WorktreeChangesResponse | null,
    headInfo: RefInfo | null
  ): WorkspaceState {
    const changes: FileChange[] = [];
    const stagedChanges: FileChange[] = [];

    const treeChanges: TreeChange[] = worktree?.changes || [];
    const assignments: HunkAssignment[] = worktree?.assignments || [];

    // Build assignment map: path -> stackId
    const assignmentMap = new Map<string, string | null>();
    for (const assignment of assignments) {
      assignmentMap.set(assignment.path, assignment.stackId);
    }

    for (const treeChange of treeChanges) {
      const changeType = treeStatusToChangeType(treeChange.status);
      const stackId = assignmentMap.get(treeChange.path) ?? null;
      const isStaged = stackId !== null;

      const fileChange: FileChange = {
        path: treeChange.path,
        previousPath:
          treeChange.status.type === 'Rename'
            ? treeChange.status.subject.previousPath
            : undefined,
        type: changeType,
        treeChange,
        stackId,
        staged: isStaged,
      };

      if (isStaged) {
        stagedChanges.push(fileChange);
      } else {
        changes.push(fileChange);
      }
    }

    return {
      changes,
      stagedChanges,
      stacks: headInfo?.stacks || [],
      target: headInfo?.target || null,
      refInfo: headInfo || null,
      isReady: true,
      rawAssignments: assignments,
      rawChanges: treeChanges,
    };
  }

  dispose(): void {
    if (this.refreshDebounce) {
      clearTimeout(this.refreshDebounce);
    }
    for (const d of this.disposables) {
      d.dispose();
    }
    this._onDidChangeState.dispose();
  }
}

export interface WorkspaceState {
  changes: FileChange[];
  stagedChanges: FileChange[];
  stacks: Stack[];
  target: Target | null;
  refInfo: RefInfo | null;
  isReady: boolean;
  /** Raw assignments from the API (needed for staging operations) */
  rawAssignments: HunkAssignment[];
  /** Raw tree changes from the API (needed for commit/discard operations) */
  rawChanges: TreeChange[];
}
