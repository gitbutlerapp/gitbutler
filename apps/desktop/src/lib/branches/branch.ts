import { type Author, Commit, DetailedCommit } from '$lib/commits/commit';
import { SkippedFile } from '$lib/files/file';
import { LocalFile } from '$lib/files/file';
import { Type, Transform, plainToInstance } from 'class-transformer';
import type { PullRequest } from '$lib/forge/interface/types';
import 'reflect-metadata';

export class VirtualBranches {
	@Type(() => BranchStack)
	branches!: BranchStack[];
	@Type(() => SkippedFile)
	skippedFiles!: SkippedFile[];
	@Type(() => DependencyError)
	dependencyErrors!: DependencyError[];
}

export function isPatchSeries(item: PatchSeries | Error): item is PatchSeries {
	return item instanceof PatchSeries;
}

export class BranchStack {
	id!: string;
	name!: string;
	notes!: string;
	@Type(() => LocalFile)
	files!: LocalFile[];
	requiresForce!: boolean;
	description!: string;
	head!: string;
	order!: number;
	@Type(() => BranchData)
	upstream?: BranchData;
	upstreamData?: BranchData;
	upstreamName?: string;
	conflicted!: boolean;
	// TODO: to be removed from the API
	baseCurrent!: boolean;
	ownership!: string;
	// This should actually be named "canBeCleanlyApplied" - if it's false, applying this branch will generate conflict markers,
	// but it's totatlly okay for a user to apply it.
	// If the branch has been already applied, then it was either performed cleanly or we generated conflict markers in the diffs.
	// (therefore this field is applicable for stashed/unapplied or remote branches, i.e. active === false)
	isMergeable!: Promise<boolean>;
	@Transform((obj) => new Date(obj.value))
	updatedAt!: Date;
	// Indicates that branch is default target for new changes
	selectedForChanges!: boolean;
	/// The merge base between the target branch and the virtual branch
	mergeBase!: string;
	/// The fork point between the target branch and the virtual branch
	forkPoint!: string;
	allowRebasing!: boolean;
	pr?: PullRequest;
	refname!: string;
	tree!: string;

	/**
	 * @desc Used in the stacking context where VirtualBranch === Stack
	 * @warning You probably want 'validSeries' instead
	 */
	@Transform(({ value }) => transformResultToType(PatchSeries, value))
	series!: (PatchSeries | Error)[];

	get validSeries(): PatchSeries[] {
		return this.series.filter(isPatchSeries);
	}

	get displayName() {
		if (this.upstream?.displayName) return this.upstream?.displayName;

		return this.upstreamName || this.name;
	}
}
export class BranchData {
	sha!: string;
	name!: string;
	upstream?: string;
	behind!: number;
	@Type(() => Commit)
	commits!: Commit[];
	isMergeable!: boolean | undefined;
	forkPoint?: string | undefined;
	isRemote!: boolean;
	givenName!: string;

	get ahead(): number {
		return this.commits.length;
	}

	get lastCommitTs(): Date | undefined {
		return this.commits[0]?.createdAt;
	}

	get firstCommitAt(): Date {
		return this.commits.at(-1)?.createdAt ?? new Date();
	}

	get authors(): Author[] {
		const allAuthors = this.commits.map((commit) => commit.author);
		const uniqueAuthors = allAuthors.filter(
			(author, index) => allAuthors.findIndex((a) => a.email === author.email) === index
		);
		return uniqueAuthors;
	}

	get displayName(): string {
		return this.name.replace('refs/remotes/', '').replace('origin/', '').replace('refs/heads/', '');
	}
}
export class PatchSeries {
	name!: string;
	description?: string;
	upstreamReference?: string;

	@Type(() => DetailedCommit)
	patches!: DetailedCommit[];
	@Type(() => DetailedCommit)
	upstreamPatches!: DetailedCommit[];

	/**
	 * A list of identifiers for the review unit at possible forges (eg. Pull Request).
	 * The list is empty if there is no review units, eg. no Pull Request has been created.
	 */
	prNumber?: number | null;
	/**
	 * Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
	 * This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
	 */
	archived!: boolean;

	get localCommits() {
		return this.patches.filter((c) => c.status === 'local');
	}

	get remoteCommits() {
		return this.patches.filter((c) => c.status === 'localAndRemote');
	}

	get integratedCommits() {
		return this.patches.filter((c) => c.status === 'integrated');
	}

	get branchName() {
		return this.name?.replace('refs/remotes/origin/', '');
	}

	get conflicted() {
		return this.patches.some((c) => c.conflicted);
	}

	get integrated() {
		return this.patches.length > 0 && this.patches.length === this.integratedCommits.length;
	}

	get ancestorMostConflictedCommit(): DetailedCommit | undefined {
		if (this.patches.length === 0) return undefined;

		for (let i = this.patches.length - 1; i >= 0; i--) {
			const commit = this.patches[i];
			if (commit?.conflicted) return commit;
		}
	}
}
/**
 * @desc Represents the order of series (branches) and changes (commits) in a stack.
 * @property series - The series are ordered from newest to oldest (most recent stacks go first).
 */

export class StackOrder {
	series!: SeriesOrder[];
}
/**
 * @desc Represents the order of changes (commits) in a series (branch).
 * @property name - Unique name of the series (branch). Must already exist in the stack.
 * @property commitIds - This is the desired commit order for the series. Because the commits will be rabased, naturally, the the commit ids will be different afte updating. The changes are ordered from newest to oldest (most recent changes go first)
 */

export class SeriesOrder {
	name!: string;
	commitIds!: string[];
}
/**
 * Represents an error that occurred when calculating dependencies for a given file change.
 */

export class DependencyError {
	errorMessage!: string;
	stackId!: string;
	commitId!: string;
	path!: string;
}
export function transformResultToType(type: any, value: any) {
	if (!Array.isArray(value)) return plainToInstance(type, value);

	return value.map((item) => {
		if ('Ok' in item) {
			return plainToInstance(type, item.Ok);
		}
		if ('Err' in item) {
			return new Error(item.Err.description);
		}
		return plainToInstance(type, item);
	});
}
