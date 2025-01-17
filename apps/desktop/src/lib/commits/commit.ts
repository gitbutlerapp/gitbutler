import { ConflictEntries } from '$lib/files/conflicts';
import { splitMessage } from '$lib/utils/commitMessage';
import { Transform } from 'class-transformer';
import 'reflect-metadata';

export class DetailedCommit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	isRemote!: boolean;
	isLocalAndRemote!: boolean;
	isIntegrated!: boolean;
	parentIds!: string[];
	branchId!: string;
	changeId!: string;
	isSigned!: boolean;
	relatedTo?: DetailedCommit;
	conflicted!: boolean;
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

	@Transform(
		(obj) =>
			new ConflictEntries(obj.value.ancestorEntries, obj.value.ourEntries, obj.value.theirEntries)
	)
	conflictedFiles!: ConflictEntries;

	// Dependency tracking
	/**
	 * The commit ids of the dependencies of this commit.
	 */
	dependencies!: string[];
	/**
	 * The ids of the commits that depend on this commit.
	 */
	reverseDependencies!: string[];
	/**
	 * The hunk hashes of uncommitted changes that depend on this commit.
	 */
	dependentDiffs!: string[];

	get status(): CommitStatus {
		if (this.isIntegrated) return 'integrated';
		if (this.isLocalAndRemote) return 'localAndRemote';
		if (this.isRemote) return 'remote';
		return 'local';
	}

	get descriptionTitle(): string | undefined {
		return splitMessage(this.description).title || undefined;
	}

	get descriptionBody(): string | undefined {
		return splitMessage(this.description).description || undefined;
	}

	isParentOf(possibleChild: DetailedCommit) {
		return possibleChild.parentIds.includes(this.id);
	}

	isMergeCommit() {
		return this.parentIds.length > 1;
	}
}

export class Commit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value * 1000))
	createdAt!: Date;
	changeId!: string;
	isSigned!: boolean;
	parentIds!: string[];
	conflicted!: boolean;

	prev?: Commit;
	next?: Commit;
	relatedTo?: DetailedCommit;

	get descriptionTitle(): string | undefined {
		return splitMessage(this.description).title || undefined;
	}

	get descriptionBody(): string | undefined {
		return splitMessage(this.description).description || undefined;
	}

	get status(): CommitStatus {
		return 'remote';
	}

	isMergeCommit() {
		return this.parentIds.length > 1;
	}

	get conflictedFiles() {
		return new ConflictEntries([], [], []);
	}
}

export type AnyCommit = DetailedCommit | Commit;

export interface Author {
	email?: string;
	name?: string;
	gravatarUrl?: string;
	isBot?: boolean;
}
export type CommitStatus = 'local' | 'localAndRemote' | 'integrated' | 'remote';
