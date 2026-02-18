/**
 * Types adapted from @gitbutler/core generated types.
 * These mirror the Rust structs exposed by the `but` CLI via `--json` output.
 */

// ---------------------------------------------------------------------------
// Tree changes (worktree status)
// ---------------------------------------------------------------------------

export type ChangeState = {
  id: string;
  kind: 'Tree' | 'Blob' | 'BlobExecutable' | 'Link' | 'Commit';
};

export type ModeFlags =
  | 'ExecutableBitAdded'
  | 'ExecutableBitRemoved'
  | 'TypeChangeFileToLink'
  | 'TypeChangeLinkToFile'
  | 'TypeChange';

export type TreeChange = {
  path: string;
  pathBytes: number[];
  status: TreeStatus;
};

export type TreeStatus =
  | { type: 'Addition'; subject: { state: ChangeState; isUntracked: boolean } }
  | { type: 'Deletion'; subject: { previousState: ChangeState } }
  | {
      type: 'Modification';
      subject: { previousState: ChangeState; state: ChangeState; flags: ModeFlags | null };
    }
  | {
      type: 'Rename';
      subject: {
        previousPath: string;
        previousPathBytes: number[];
        previousState: ChangeState;
        state: ChangeState;
        flags: ModeFlags | null;
      };
    };

// ---------------------------------------------------------------------------
// Hunk assignment
// ---------------------------------------------------------------------------

export type HunkHeader = {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
};

export type HunkAssignment = {
  id: string | null;
  hunkHeader: HunkHeader | null;
  path: string;
  pathBytes: number[];
  stackId: string | null;
  lineNumsAdded: number[] | null;
  lineNumsRemoved: number[] | null;
};

// ---------------------------------------------------------------------------
// Commits & branches
// ---------------------------------------------------------------------------

export type Author = {
  name: string;
  email: string;
  gravatarUrl: string;
};

export type CommitState =
  | { type: 'LocalOnly' }
  | { type: 'LocalAndRemote'; subject: string }
  | { type: 'Integrated' };

export type Commit = {
  id: string;
  parentIds: string[];
  message: string;
  hasConflicts: boolean;
  state: CommitState;
  createdAt: number;
  author: Author;
};

export type UpstreamCommit = {
  id: string;
  message: string;
  createdAt: number;
  author: Author;
};

export type PushStatus =
  | 'nothingToPush'
  | 'unpushedCommits'
  | 'unpushedCommitsRequiringForce'
  | 'completelyUnpushed'
  | 'integrated';

export type BranchDetails = {
  name: string;
  reference: string;
  linkedWorktreeId: string | null;
  remoteTrackingBranch: string | null;
  prNumber: number | null;
  reviewId: string | null;
  tip: string;
  baseCommit: string;
  pushStatus: PushStatus;
  lastUpdatedAt: number | null;
  authors: Author[];
  isConflicted: boolean;
  commits: Commit[];
  upstreamCommits: UpstreamCommit[];
  isRemoteHead: boolean;
};

export type StackDetails = {
  derivedName: string;
  pushStatus: PushStatus;
  branchDetails: BranchDetails[];
  isConflicted: boolean;
};

// ---------------------------------------------------------------------------
// Workspace ref info (from `but status --json`)
// ---------------------------------------------------------------------------

export type BranchReference = {
  fullNameBytes: number[];
  displayName: string;
};

export type RemoteTrackingReference = {
  fullNameBytes: number[];
  displayName: string;
  remoteName: string;
};

export type Segment = {
  refName: BranchReference | null;
  remoteTrackingRefName: RemoteTrackingReference | null;
  commits: Commit[];
  commitsOnRemote: UpstreamCommit[];
  pushStatus: PushStatus;
  base: string | null;
  isEntrypoint: boolean;
};

export type Stack = {
  id: string | null;
  base: string | null;
  segments: Segment[];
};

export type Target = {
  remoteTrackingRef: RemoteTrackingReference;
  commitsAhead: number;
};

export type RefInfo = {
  workspaceRef: BranchReference | null;
  stacks: Stack[];
  target: Target | null;
  isManagedRef: boolean;
  isManagedCommit: boolean;
  isEntrypoint: boolean;
};

// ---------------------------------------------------------------------------
// CLI output shapes (what `but --json` commands return)
// ---------------------------------------------------------------------------

/** Represents the parsed JSON output of `but status --json` */
export interface ButStatus {
  refInfo?: RefInfo;
  stacks?: Stack[];
  changes?: TreeChange[];
  assignments?: HunkAssignment[];
  // The CLI output shape may vary; we handle what we get
  [key: string]: unknown;
}

/** Branch list entry from `but branch list --json` */
export interface BranchListEntry {
  name: string;
  isLocal: boolean;
  isRemote: boolean;
  isApplied?: boolean;
  ahead?: number;
  behind?: number;
  lastUpdated?: string;
  [key: string]: unknown;
}

/** Output of `but diff --json` */
export interface DiffOutput {
  path: string;
  hunks: DiffHunk[];
  [key: string]: unknown;
}

export interface DiffHunk {
  header: HunkHeader;
  diff: string;
  lines: DiffLine[];
}

export interface DiffLine {
  type: 'context' | 'addition' | 'deletion';
  content: string;
  oldLineNo?: number;
  newLineNo?: number;
}

// ---------------------------------------------------------------------------
// Internal extension types
// ---------------------------------------------------------------------------

export type FileChangeType = 'added' | 'modified' | 'deleted' | 'renamed' | 'untracked';

export interface FileChange {
  /** Workspace-relative path */
  path: string;
  /** Previous path (for renames) */
  previousPath?: string;
  /** Type of change */
  type: FileChangeType;
  /** The raw TreeChange from the CLI */
  treeChange: TreeChange;
  /** Which stack/branch this change is assigned to (null = unassigned) */
  stackId: string | null;
  /** Whether this change is staged */
  staged: boolean;
}

export function treeStatusToChangeType(status: TreeStatus): FileChangeType {
  switch (status.type) {
    case 'Addition':
      return status.subject.isUntracked ? 'untracked' : 'added';
    case 'Deletion':
      return 'deleted';
    case 'Modification':
      return 'modified';
    case 'Rename':
      return 'renamed';
  }
}
