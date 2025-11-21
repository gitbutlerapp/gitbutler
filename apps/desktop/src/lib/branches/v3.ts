import type { TreeChange, TreeStats } from '$lib/hunks/change';
import type { Workspace } from '@gitbutler/core/api';

/** Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.*/
export type Commit = Workspace.Commit;

/** List of changes, stats and metadata for a commit */
export type CommitDetails = {
	/** The commit */
	readonly commit: Commit;
	/** The changes that were made to the tree. */
	readonly changes: TreeChange[];
	/** The stats of the changes. */
	readonly stats: TreeStats;
	/** If there are any conflicted files this will show them */
	readonly conflictEntries?: {
		ancestorEntries: string[];
		ourEntries: string[];
		theirEntries: string[];
	};
};

/** Safely extract the creation time in milliseconds */
export function commitCreatedAt(commit: Commit | UpstreamCommit): number {
	return Number(commit.createdAt);
}

/** Safely extract the creation date from the commit */
export function commitCreatedAtDate(commit: Commit | UpstreamCommit): Date {
	return new Date(commitCreatedAt(commit));
}

/** If the commit is in `LocalAndRemote` state, extract the subject (the remote commit ID) */
export function commitStateSubject(commit: Commit): string | null {
	switch (commit.state.type) {
		case 'LocalOnly':
			return null;
		case 'Integrated':
			return null;
		case 'LocalAndRemote':
			return commit.state.subject;
	}
}

/**
 * Commit that is only at the remote.
 * Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
 */
export type UpstreamCommit = Workspace.UpstreamCommit;

export function isCommit(something: Commit | UpstreamCommit): something is Commit {
	return 'state' in something;
}

export function extractUpstreamCommitId(commit: Commit | UpstreamCommit): string | undefined {
	if (isCommit(commit)) {
		if (commit.state.type === 'LocalAndRemote') {
			return commit.state.subject;
		}
	}
	return undefined;
}

/** Represents the author of a commit. */
export type Author = Workspace.Author;

/** Represents the state a commit could be in. */
export type CommitState = Workspace.CommitState;
