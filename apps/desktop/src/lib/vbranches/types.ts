import 'reflect-metadata';
import { splitMessage } from '$lib/utils/commitMessage';
import { hashCode } from '$lib/utils/string';
import { isDefined, notNull } from '@gitbutler/ui/utils/typeguards';
import { Type, Transform } from 'class-transformer';
import type { PullRequest } from '$lib/gitHost/interface/types';

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
		return this.hunks
			.flatMap((hunk) => hunk.lockedTo)
			.filter(notNull)
			.filter(isDefined);
	}

	get looksConflicted(): boolean {
		return fileLooksConflicted(this);
	}
}

export class SkippedFile {
	oldPath!: string | undefined;
	newPath!: string | undefined;
	binary!: boolean;
	oldSizeBytes!: number;
	newSizeBytes!: number;
}

export class VirtualBranches {
	@Type(() => VirtualBranch)
	branches!: VirtualBranch[];
	@Type(() => SkippedFile)
	skippedFiles!: SkippedFile[];
}

export class VirtualBranch {
	id!: string;
	name!: string;
	notes!: string;
	@Type(() => LocalFile)
	files!: LocalFile[];
	@Type(() => DetailedCommit)
	commits!: DetailedCommit[];
	requiresForce!: boolean;
	description!: string;
	head!: string;
	order!: number;
	@Type(() => Branch)
	upstream?: Branch;
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

	get localCommits() {
		return this.commits.filter((c) => c.status === 'local');
	}

	get remoteCommits() {
		return this.commits.filter((c) => c.status === 'localAndRemote');
	}

	get integratedCommits() {
		return this.commits.filter((c) => c.status === 'integrated');
	}

	get displayName() {
		if (this.upstream?.displayName) return this.upstream?.displayName;

		return this.upstreamName || this.name;
	}
}

// Used for dependency injection
export const BRANCH = Symbol('branch');
export type CommitStatus = 'local' | 'localAndRemote' | 'integrated' | 'remote';

export class DetailedCommit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	isRemote!: boolean;
	isIntegrated!: boolean;
	@Type(() => LocalFile)
	files!: LocalFile[];
	parentIds!: string[];
	branchId!: string;
	changeId!: string;
	isSigned!: boolean;
	relatedTo?: Commit;
	conflicted!: boolean;
	// Set if a GitButler branch reference pointing to this commit exists. In the format of "refs/remotes/origin/my-branch"
	remoteRef?: string | undefined;
	// Identifies the remote commit id from which this local commit was copied. The backend figured this out by comparing
	// author, commit and message.
	copiedFromRemoteId?: string;

	prev?: DetailedCommit;
	next?: DetailedCommit;

	get status(): CommitStatus {
		if (this.isIntegrated) return 'integrated';
		if (this.isRemote && (!this.relatedTo || this.id === this.relatedTo.id))
			return 'localAndRemote';
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
}

export type AnyCommit = DetailedCommit | Commit;

export function commitCompare(left: AnyCommit, right: DetailedCommit): boolean {
	if (left.id === right.id) return true;
	if (left.changeId && right.changeId && left.changeId === right.changeId) return true;
	if (right.copiedFromRemoteId && right.copiedFromRemoteId === left.id) return true;
	return false;
}

export class RemoteHunk {
	diff!: string;
	hash?: string;
	new_start!: number;
	new_lines!: number;

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

	get id(): string {
		return this.path;
	}

	get filename(): string {
		return this.path.replace(/^.*[\\/]/, '');
	}

	get justpath() {
		return this.path.split('/').slice(0, -1).join('/');
	}

	get large() {
		return false;
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

	get looksConflicted(): boolean {
		return fileLooksConflicted(this);
	}
}

function fileLooksConflicted(file: AnyFile) {
	const hasStartingMarker = file.hunks.some((hunk) =>
		hunk.diff.split('\n').some((line) => line.startsWith('>>>>>>> theirs', 1))
	);

	const hasEndingMarker = file.hunks.some((hunk) =>
		hunk.diff.split('\n').some((line) => line.startsWith('<<<<<<< ours', 1))
	);

	return hasStartingMarker && hasEndingMarker;
}

export interface Author {
	email?: string;
	name?: string;
	gravatarUrl?: string;
	isBot?: boolean;
}

export class Branch {
	sha!: string;
	name!: string;
	upstream?: string;
	lastCommitTimestampMs?: number | undefined;
	lastCommitAuthor?: string | undefined;
	givenName!: string;
	isRemote!: boolean;

	get displayName(): string {
		return this.name.replace('refs/remotes/', '').replace('refs/heads/', '');
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
