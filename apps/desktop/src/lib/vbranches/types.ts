import 'reflect-metadata';
import { emptyConflictEntryPresence, type ConflictEntryPresence } from '$lib/conflictEntryPresence';
import { splitMessage } from '$lib/utils/commitMessage';
import { hashCode } from '@gitbutler/ui/utils/string';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { Type, Transform, plainToInstance } from 'class-transformer';
import type { PullRequest } from '$lib/forge/interface/types';

function transformResultToType(type: any, value: any) {
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

export type ChangeType =
	/// Entry does not exist in old version
	| 'added'
	/// Entry does not exist in new version
	| 'deleted'
	/// Entry content changed between old and new
	| 'modified';

export class Hunk {
	id!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
	hash?: string;
	locked!: boolean;
	@Type(() => HunkLock)
	lockedTo!: HunkLock[];
	/// Indicates that the hunk depends on multiple branches. In this case the hunk cant be moved or comitted.
	poisoned!: boolean;
	changeType!: ChangeType;
	new_start!: number;
	new_lines!: number;
}

export class HunkLock {
	branchId!: string;
	commitId!: string;
}

export type AnyFile = LocalFile | RemoteFile;

export function isAnyFile(something: unknown): something is AnyFile {
	return something instanceof LocalFile || something instanceof RemoteFile;
}

export class LocalFile {
	id!: string;
	path!: string;
	@Type(() => Hunk)
	hunks!: Hunk[];
	expanded?: boolean;
	@Transform((obj) => new Date(obj.value))
	modifiedAt!: Date;
	// This indicates if a file has merge conflict markers generated and not yet resolved.
	// This is true for files after a branch which does not apply cleanly (Branch.isMergeable === false) is applied.
	// (therefore this field is applicable only for the workspace, i.e. active === true)
	conflicted!: boolean;
	content!: string;
	binary!: boolean;
	large!: boolean;

	get filename(): string {
		const parts = this.path.split('/');
		return parts.at(-1) ?? this.path;
	}

	get justpath() {
		return this.path.split('/').slice(0, -1).join('/');
	}

	get hunkIds() {
		return this.hunks.map((h) => h.id);
	}

	get locked(): boolean {
		return this.hunks
			? this.hunks.map((hunk) => hunk.locked).reduce((a, b) => !!(a || b), false)
			: false;
	}

	get lockedIds(): HunkLock[] {
		return this.hunks.flatMap((hunk) => hunk.lockedTo).filter(isDefined);
	}
}

export class SkippedFile {
	oldPath!: string | undefined;
	newPath!: string | undefined;
	binary!: boolean;
	oldSizeBytes!: number;
	newSizeBytes!: number;
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

export type CommitStatus = 'local' | 'localAndRemote' | 'integrated' | 'remote';

export class ConflictEntries {
	public entries: Map<string, ConflictEntryPresence> = new Map();
	constructor(ancestorEntries: string[], ourEntries: string[], theirEntries: string[]) {
		ancestorEntries.forEach((entry) => {
			const entryPresence = this.entries.get(entry) || emptyConflictEntryPresence();
			entryPresence.ancestor = true;
			this.entries.set(entry, entryPresence);
		});
		ourEntries.forEach((entry) => {
			const entryPresence = this.entries.get(entry) || emptyConflictEntryPresence();
			entryPresence.ours = true;
			this.entries.set(entry, entryPresence);
		});
		theirEntries.forEach((entry) => {
			const entryPresence = this.entries.get(entry) || emptyConflictEntryPresence();
			entryPresence.theirs = true;
			this.entries.set(entry, entryPresence);
		});
	}
}

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

export class RemoteHunk {
	diff!: string;
	hash?: string;
	new_start!: number;
	new_lines!: number;
	changeType!: ChangeType;

	get id(): string {
		return hashCode(this.diff);
	}

	get locked() {
		return false;
	}
}

export class RemoteFile {
	path!: string;
	@Type(() => RemoteHunk)
	hunks!: RemoteHunk[];
	binary!: boolean;
	large!: boolean;

	get id(): string {
		return 'remote:' + this.path;
	}

	get filename(): string {
		return this.path.replace(/^.*[\\/]/, '');
	}

	get justpath() {
		return this.path.split('/').slice(0, -1).join('/');
	}

	get conflicted() {
		return false;
	}

	get hunkIds() {
		return this.hunks.map((h) => h.id);
	}

	get lockedIds(): HunkLock[] {
		return [];
	}

	get locked(): boolean {
		return false;
	}
}

export interface Author {
	email?: string;
	name?: string;
	gravatarUrl?: string;
	isBot?: boolean;
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

export interface BranchPushResult {
	refname: string;
	remote: string;
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
 * @desc Represents a GitHub Pull Request identifier.
 * @property prNumber - The GitHub Pull Request identifier.
 */
export interface GitHubIdentifier {
	prNumber: number;
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
