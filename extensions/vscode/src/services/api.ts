import * as vscode from 'vscode';
import * as http from 'http';
import type { ServerManager } from './server';
import type {
  TreeChange,
  HunkAssignment,
  StackDetails,
  BranchDetails,
  Stack,
  RefInfo,
  HunkHeader,
} from '../types/gitbutler';

/**
 * Response shape from but-server.
 * All endpoints return `{ "type": "success", "subject": T }` or `{ "type": "error", "subject": E }`.
 */
interface ServerResponse<T = unknown> {
  type: 'success' | 'error';
  subject: T;
}

/**
 * HTTP client for the but-server API.
 * Mirrors the protocol used by the GitButler web frontend.
 */
export class ButApiClient implements vscode.Disposable {
  private outputChannel: vscode.OutputChannel;
  private _projectId: string | null = null;

  get projectId(): string | null {
    return this._projectId;
  }

  constructor(
    private readonly server: ServerManager,
    outputChannel: vscode.OutputChannel
  ) {
    this.outputChannel = outputChannel;
  }

  dispose(): void {}

  // -------------------------------------------------------------------------
  // Core HTTP transport
  // -------------------------------------------------------------------------

  /**
   * POST to a but-server endpoint and return the parsed response subject.
   */
  async invoke<T = unknown>(
    command: string,
    params: Record<string, unknown> = {}
  ): Promise<T> {
    const url = `${this.server.baseUrl}/${command}`;
    const body = JSON.stringify(params);

    this.outputChannel.appendLine(`POST /${command} ${body.substring(0, 200)}`);

    const responseText = await this.httpPost(url, body);
    let parsed: ServerResponse<T>;
    try {
      parsed = JSON.parse(responseText) as ServerResponse<T>;
    } catch {
      throw new ApiError(command, `Invalid JSON response: ${responseText.substring(0, 200)}`);
    }

    if (parsed.type === 'success') {
      return parsed.subject;
    } else {
      const errMsg =
        typeof parsed.subject === 'object' && parsed.subject !== null
          ? JSON.stringify(parsed.subject)
          : String(parsed.subject);
      throw new ApiError(command, errMsg);
    }
  }

  /**
   * Convenience: invoke with projectId auto-injected.
   */
  async call<T = unknown>(
    command: string,
    params: Record<string, unknown> = {}
  ): Promise<T> {
    if (!this._projectId) {
      throw new ApiError(command, 'No project registered. Call registerProject first.');
    }
    return this.invoke<T>(command, { projectId: this._projectId, ...params });
  }

  private httpPost(url: string, body: string): Promise<string> {
    return new Promise((resolve, reject) => {
      const req = http.request(
        url,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Content-Length': Buffer.byteLength(body),
          },
          timeout: 30000,
        },
        (res) => {
          const chunks: Buffer[] = [];
          res.on('data', (chunk: Buffer) => chunks.push(chunk));
          res.on('end', () => resolve(Buffer.concat(chunks).toString('utf-8')));
        }
      );
      req.on('error', (err) => reject(new ApiError('http', err.message)));
      req.on('timeout', () => {
        req.destroy();
        reject(new ApiError('http', 'Request timed out'));
      });
      req.write(body);
      req.end();
    });
  }

  // -------------------------------------------------------------------------
  // Project management
  // -------------------------------------------------------------------------

  /**
   * Register a project with but-server. Must be called before any other API.
   * Returns the project ID (UUID).
   */
  async registerProject(repoPath: string): Promise<string> {
    const result = await this.invoke<{ project_id?: string; projectId?: string; id?: string } | any>(
      'add_project_best_effort',
      { path: repoPath }
    );

    // The response shape may vary — extract the project ID
    let id: string | undefined;
    if (typeof result === 'object' && result !== null) {
      id = result.projectId || result.project_id || result.id;
      // The AddProjectOutcome may be nested
      if (!id && result.project) {
        id = result.project.id;
      }
      // It might also be the full project object with an `id` field
      if (!id && typeof result === 'object') {
        // Walk through possible shapes
        for (const key of Object.keys(result)) {
          const val = (result as Record<string, unknown>)[key];
          if (typeof val === 'object' && val !== null && 'id' in val) {
            id = (val as { id: string }).id;
            break;
          }
        }
      }
    }
    if (typeof result === 'string') {
      id = result;
    }

    if (!id) {
      this.outputChannel.appendLine(
        `registerProject: Could not extract ID from response: ${JSON.stringify(result)}`
      );
      throw new ApiError('add_project_best_effort', 'Could not extract project ID from response');
    }

    this._projectId = id;
    this.outputChannel.appendLine(`Project registered: ${id}`);
    return id;
  }

  // -------------------------------------------------------------------------
  // Workspace / Status
  // -------------------------------------------------------------------------

  /** Get ref info (stacks, target, workspace ref). */
  async headInfo(): Promise<RefInfo> {
    return this.call<RefInfo>('head_info');
  }

  /** Get stacks in the workspace. */
  async stacks(filter?: 'All' | 'InWorkspace' | 'Unapplied'): Promise<StackEntry[]> {
    return this.call<StackEntry[]>('stacks', { filter: filter || null });
  }

  /** Get worktree changes (uncommitted files + hunk assignments). */
  async changesInWorktree(): Promise<WorktreeChangesResponse> {
    return this.call<WorktreeChangesResponse>('changes_in_worktree');
  }

  /** Get stack details (branches, commits). */
  async stackDetails(stackId: string): Promise<StackDetails> {
    return this.call<StackDetails>('stack_details', { stackId });
  }

  /** Get branch details. */
  async branchDetails(branchName: string, stackId: string): Promise<BranchDetails> {
    return this.call<BranchDetails>('branch_details', { branchName, stackId });
  }

  /** Get diff for a specific tree change. */
  async treeChangeDiffs(change: TreeChange): Promise<unknown> {
    return this.call('tree_change_diffs', { change });
  }

  // -------------------------------------------------------------------------
  // Staging / Hunk assignment
  // -------------------------------------------------------------------------

  /**
   * Assign hunks to a stack (stage) or unassign them (unstage).
   * stackId = null means unassign.
   */
  async assignHunk(
    assignments: Array<{
      hunkHeader: HunkHeader | null;
      pathBytes: number[];
      stackId: string | null;
    }>
  ): Promise<unknown[]> {
    return this.call<unknown[]>('assign_hunk', { assignments });
  }

  // -------------------------------------------------------------------------
  // Committing
  // -------------------------------------------------------------------------

  /** Create a commit from worktree changes. */
  async createCommit(opts: {
    stackId: string;
    message: string;
    stackBranchName: string;
    worktreeChanges: DiffSpec[];
    parentId?: string | null;
  }): Promise<unknown> {
    return this.call('create_commit_from_worktree_changes', {
      stackId: opts.stackId,
      parentId: opts.parentId || null,
      worktreeChanges: opts.worktreeChanges,
      message: opts.message,
      stackBranchName: opts.stackBranchName,
    });
  }

  /** Amend a commit with worktree changes. */
  async amendCommit(opts: {
    stackId: string;
    commitId: string;
    worktreeChanges: DiffSpec[];
    message?: string;
  }): Promise<unknown> {
    return this.call('amend_commit_from_worktree_changes', opts);
  }

  // -------------------------------------------------------------------------
  // Discard
  // -------------------------------------------------------------------------

  /** Discard worktree changes. */
  async discardChanges(worktreeChanges: DiffSpec[]): Promise<unknown> {
    return this.call('discard_worktree_changes', { worktreeChanges });
  }

  // -------------------------------------------------------------------------
  // Workspace mode
  // -------------------------------------------------------------------------

  /** Get the current operating mode (OpenWorkspace, OutsideWorkspace, etc). */
  async operatingMode(): Promise<OperatingModeResponse> {
    return this.call<OperatingModeResponse>('operating_mode');
  }

  /** Set the base branch to enter workspace mode. Branch should be like "origin/master". */
  async setBaseBranch(branch: string, pushRemote?: string | null): Promise<unknown> {
    return this.call('set_base_branch', {
      branch,
      pushRemote: pushRemote ?? null,
    });
  }

  /** Switch back to workspace mode (if base branch is already set). */
  async switchBackToWorkspace(): Promise<unknown> {
    return this.call('switch_back_to_workspace');
  }

  // -------------------------------------------------------------------------
  // Branch management
  // -------------------------------------------------------------------------

  /** Create a new independent virtual branch (stack). */
  async createVirtualBranch(name?: string): Promise<unknown> {
    return this.call('create_virtual_branch', {
      branch: { name: name || null, order: null },
    });
  }

  /**
   * Create a new reference (branch). This is the modern API.
   * - No anchor = new independent stack
   * - Anchor at a reference with position Above = stacked/child branch
   */
  async createReference(opts: {
    newName: string;
    anchor?: {
      type: 'AtCommit' | 'AtReference';
      subject: {
        commitId?: string;
        shortName?: string;
        position: 'Above' | 'Below';
      };
    } | null;
  }): Promise<unknown> {
    return this.call('create_reference', {
      request: {
        newName: opts.newName,
        anchor: opts.anchor ?? null,
      },
    });
  }

  /**
   * Create a dependent (child/stacked) branch on top of an existing stack.
   * Legacy API — uses stack ID.
   */
  async createStackedBranch(stackId: string, name: string): Promise<unknown> {
    return this.call('create_branch', {
      stackId,
      request: { name, precedingHead: null },
    });
  }

  /** Remove a branch from a stack. */
  async removeBranch(stackId: string, branchName: string): Promise<unknown> {
    return this.call('remove_branch', { stackId, branchName });
  }

  /** List branches. */
  async listBranches(filter?: { local?: boolean; applied?: boolean }): Promise<unknown[]> {
    return this.call<unknown[]>('list_branches', { filter: filter || null });
  }

  /** Delete a local branch. */
  async deleteLocalBranch(refname: string): Promise<unknown> {
    return this.call('delete_local_branch', { refname });
  }

  // -------------------------------------------------------------------------
  // Push / Fetch
  // -------------------------------------------------------------------------

  /** Push a stack. */
  async pushStack(opts: {
    stackId: string;
    branch: string;
    withForce?: boolean;
  }): Promise<unknown> {
    return this.call('push_stack', {
      stackId: opts.stackId,
      branch: opts.branch,
      withForce: opts.withForce ?? false,
      skipForcePushProtection: false,
      runHooks: true,
      pushOpts: [],
    });
  }

  /** Fetch from remotes. */
  async fetchFromRemotes(): Promise<unknown> {
    return this.call('fetch_from_remotes', { action: 'status' });
  }

  // -------------------------------------------------------------------------
  // Undo
  // -------------------------------------------------------------------------

  /** List snapshots. */
  async listSnapshots(): Promise<unknown[]> {
    return this.call<unknown[]>('list_snapshots', { limit: 20, sha: null });
  }

  /** Restore a snapshot. */
  async restoreSnapshot(sha: string): Promise<unknown> {
    return this.call('restore_snapshot', { sha });
  }

  // -------------------------------------------------------------------------
  // File content (for diffs)
  // -------------------------------------------------------------------------

  /** Get file content from a workspace commit (HEAD version). */
  async getWorkspaceFile(relativePath: string): Promise<string> {
    try {
      const result = await this.call<string>('get_workspace_file', {
        relativePath,
      });
      return result;
    } catch {
      return '';
    }
  }

  /** Get file content from a specific commit. */
  async getCommitFile(commitId: string, relativePath: string): Promise<string> {
    try {
      const result = await this.call<string>('get_commit_file', {
        commitOid: commitId,
        path: relativePath,
      });
      return result;
    } catch {
      return '';
    }
  }
}

export class ApiError extends Error {
  constructor(
    public readonly command: string,
    public readonly detail: string
  ) {
    super(`${command}: ${detail}`);
    this.name = 'ApiError';
  }
}

// -------------------------------------------------------------------------
// Additional response types
// -------------------------------------------------------------------------

export interface StackEntry {
  id: string;
  heads: Array<{
    name: string;
    remoteTrackingBranch: string | null;
    tip: string;
    prNumber: number | null;
  }>;
  tip: string;
  [key: string]: unknown;
}

export interface WorktreeChangesResponse {
  changes: TreeChange[];
  ignoredChanges: Array<{ path: string; status: string }>;
  assignments: HunkAssignment[];
  [key: string]: unknown;
}

export interface DiffSpec {
  previousPathBytes: number[] | null;
  pathBytes: number[];
  hunkHeaders: HunkHeader[];
}

export interface OperatingModeResponse {
  head: string;
  operatingMode: {
    type: 'OpenWorkspace' | 'OutsideWorkspace' | 'Edit';
    subject?: unknown;
  };
}

/**
 * Convert a file path string to a byte array (for pathBytes params).
 */
export function pathToBytes(filePath: string): number[] {
  return Array.from(Buffer.from(filePath, 'utf-8'));
}
