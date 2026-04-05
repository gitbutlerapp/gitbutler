import { splitMessage } from "$lib/commits/commitMessage";

export type CommitKey = {
	stackId?: string;
	branchName: string;
	commitId: string;
	upstream: boolean;
};

export interface DetailedCommit {
	id: string;
	author: Author;
	description: string;
	/** Milliseconds since epoch. */
	createdAt: number;
	isRemote: boolean;
	isLocalAndRemote: boolean;
	isIntegrated: boolean;
	parentIds: string[];
	branchId: string;
	changeId: string;
	isSigned: boolean;
	relatedTo?: DetailedCommit;
	conflicted: boolean;
	// Set if a GitButler branch reference pointing to this commit exists. In the format of "refs/remotes/origin/my-branch"
	remoteRef?: string | undefined;

	/**
	 *
	 * Represents the remote commit id of this patch.
	 * This field is set if:
	 *   - The commit has been pushed
	 *   - The commit has been copied from a remote commit (when applying a remote branch)
	 *
	 * The `remoteCommitId` may be the same as the `id` or it may be different if the commit has been rebased or updated.
	 *
	 * Note: This makes both the `isRemote` and `copiedFromRemoteId` fields redundant, but they are kept for compatibility.
	 */
	remoteCommitId?: string;

	prev?: DetailedCommit;
	next?: DetailedCommit;

	conflictedFiles: {
		ancestorEntries: string[];
		ourEntries: string[];
		theirEntries: string[];
	};

	// Dependency tracking
	/** The commit ids of the dependencies of this commit. */
	dependencies: string[];
	/** The ids of the commits that depend on this commit. */
	reverseDependencies: string[];
	/** The hunk hashes of uncommitted changes that depend on this commit. */
	dependentDiffs: string[];
}

export interface Commit {
	id: string;
	author: Author;
	description: string;
	/** Milliseconds since epoch. */
	createdAt: number;
	changeId: string;
	isSigned: boolean;
	parentIds: string[];
	conflicted: boolean;

	prev?: Commit;
	next?: Commit;
	relatedTo?: DetailedCommit;
}

export type AnyCommit = DetailedCommit | Commit;

export function commitStatus(commit: AnyCommit): CommitStatusType {
	if ("isIntegrated" in commit) {
		if (commit.isIntegrated) return "Integrated";
		if (commit.isLocalAndRemote) return "LocalAndRemote";
		if (commit.isRemote) return "Remote";
		return "LocalOnly";
	}
	return "Remote";
}

export function descriptionTitle(commit: AnyCommit): string | undefined {
	return splitMessage(commit.description).title || undefined;
}

export function descriptionBody(commit: AnyCommit): string | undefined {
	return splitMessage(commit.description).description || undefined;
}

export function isParentOf(parent: DetailedCommit, possibleChild: DetailedCommit): boolean {
	return possibleChild.parentIds.includes(parent.id);
}

export function isMergeCommit(commit: AnyCommit): boolean {
	return commit.parentIds.length > 1;
}

export interface Author {
	id?: number;
	email?: string;
	name?: string;
	gravatarUrl?: string;
	isBot?: boolean;
}

export enum CommitStatus {
	LocalOnly = "LocalOnly",
	LocalAndRemote = "LocalAndRemote",
	Integrated = "Integrated",
	Remote = "Remote",
	Base = "Base",
}

export type CommitStatusType = keyof typeof CommitStatus;

export function commitStatusLabel(status: CommitStatusType): string {
	switch (status) {
		case CommitStatus.LocalOnly:
			return "Local";
		case CommitStatus.LocalAndRemote:
			return "Local and remote";
		case CommitStatus.Integrated:
			return "Integrated";
		case CommitStatus.Remote:
			return "Remote";
		case CommitStatus.Base:
			return "Base";
		default:
			return status;
	}
}

export type MoveCommitIllegalAction =
	| {
			type: "dependsOnCommits";
			subject: string[];
	  }
	| {
			type: "hasDependentChanges";
			subject: string[];
	  }
	| {
			type: "hasDependentUncommittedChanges";
	  };

function formatCommitIds(ids: string[]): string {
	return ids.map((id) => id.slice(0, 7)).join("\n");
}

export function getMoveCommitIllegalActionMessage(action: MoveCommitIllegalAction): string {
	switch (action.type) {
		case "dependsOnCommits":
			return `Cannot move commit because it depends on the following commits: ${formatCommitIds(action.subject)}`;
		case "hasDependentChanges":
			return `Cannot move commit because it has dependent changes: ${formatCommitIds(action.subject)}`;
		case "hasDependentUncommittedChanges":
			return `Cannot move commit because it has dependent uncommitted changes`;
	}
}
