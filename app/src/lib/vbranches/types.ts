import 'reflect-metadata';
import { splitMessage } from '$lib/utils/commitMessage';
import { hashCode } from '$lib/utils/string';
import { isDefined, notNull } from '$lib/utils/typeguards';
import { convertRemoteToWebUrl } from '$lib/utils/url';
import { Type, Transform } from 'class-transformer';

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
	changeType!: ChangeType;
	new_start!: number;
	new_lines!: number;
}

export class HunkLock {
	branchId!: string;
	commitId!: string;
}

export type AnyFile = LocalFile | RemoteFile;

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
		return parts[parts.length - 1];
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
}

export class SkippedFile {
	oldPath!: string | undefined;
	newPath!: string | undefined;
	binary!: boolean;
	oldSizeBytes!: number;
	newSizeBytes!: number;
}

export class VirtualBranches {
	@Type(() => Branch)
	branches!: Branch[];
	@Type(() => SkippedFile)
	skippedFiles!: SkippedFile[];
}

export class Branch {
	id!: string;
	name!: string;
	notes!: string;
	// Active means the branch has been applied to the workspace
	active!: boolean;
	@Type(() => LocalFile)
	files!: LocalFile[];
	@Type(() => Commit)
	commits!: Commit[];
	requiresForce!: boolean;
	description!: string;
	head!: string;
	order!: number;
	@Type(() => RemoteBranch)
	upstream?: RemoteBranch;
	upstreamData?: RemoteBranchData;
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
export type ComponentStyleKind = 'solid' | 'soft';
export type ComponentColor =
	| 'neutral'
	| 'ghost'
	| 'pop'
	| 'success'
	| 'error'
	| 'warning'
	| 'purple';
export type CommitStatus = 'local' | 'localAndRemote' | 'integrated' | 'remote';

export class Commit {
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
	relatedTo?: RemoteCommit;

	prev?: Commit;
	next?: Commit;

	get isLocal() {
		return !this.isRemote && !this.isIntegrated;
	}

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

	isParentOf(possibleChild: Commit) {
		return possibleChild.parentIds.includes(this.id);
	}

	isMergeCommit() {
		return this.parentIds.length > 1;
	}
}

export function isLocalCommit(obj: any): obj is Commit {
	return obj instanceof Commit;
}

export class RemoteCommit {
	id!: string;
	author!: Author;
	description!: string;
	@Transform((obj) => new Date(obj.value * 1000))
	createdAt!: Date;
	changeId!: string;
	isSigned!: boolean;
	parentIds!: string[];

	prev?: RemoteCommit;
	next?: RemoteCommit;
	relatedTo?: Commit;

	get isLocal() {
		return false;
	}

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

export function isRemoteCommit(obj: any): obj is RemoteCommit {
	return obj instanceof RemoteCommit;
}

export type AnyCommit = Commit | RemoteCommit;

export const LOCAL_COMMITS = Symbol('LocalCommtis');
export const REMOTE_COMMITS = Symbol('RemoteCommits');
export const INTEGRATED_COMMITS = Symbol('IntegratedCommits');
export const UNKNOWN_COMMITS = Symbol('UnknownCommits');

export function commitCompare(left: AnyCommit, right: AnyCommit): boolean {
	if (left.id === right.id) return true;
	if (left.changeId && right.changeId && left.changeId === right.changeId) return true;
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
}

export interface Author {
	email?: string;
	name?: string;
	gravatarUrl?: URL;
	isBot?: boolean;
}

export class RemoteBranch {
	sha!: string;
	name!: string;
	upstream?: string;
	lastCommitTimestampMs?: number | undefined;
	lastCommitAuthor?: string | undefined;

	get displayName(): string {
		return this.name.replace('refs/remotes/', '').replace('refs/heads/', '');
	}
}

export class RemoteBranchData {
	sha!: string;
	name!: string;
	upstream?: string;
	behind!: number;
	@Type(() => RemoteCommit)
	commits!: RemoteCommit[];
	isMergeable!: boolean | undefined;
	forkPoint?: string | undefined;

	get ahead(): number {
		return this.commits.length;
	}

	get lastCommitTs(): Date | undefined {
		return this.commits[0]?.createdAt;
	}

	get firstCommitAt(): Date {
		return this.commits[this.commits.length - 1].createdAt;
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

export class BaseBranch {
	branchName!: string;
	remoteName!: string;
	remoteUrl!: string;
	pushRemoteName!: string;
	pushRemoteUrl!: string;
	baseSha!: string;
	currentSha!: string;
	behind!: number;
	@Type(() => RemoteCommit)
	upstreamCommits!: RemoteCommit[];
	@Type(() => RemoteCommit)
	recentCommits!: RemoteCommit[];
	lastFetchedMs?: number;

	actualPushRemoteName(): string {
		return this.pushRemoteName || this.remoteName;
	}

	get lastFetched(): Date | undefined {
		return this.lastFetchedMs ? new Date(this.lastFetchedMs) : undefined;
	}

	get pushRepoBaseUrl(): string {
		return convertRemoteToWebUrl(this.pushRemoteUrl);
	}

	get repoBaseUrl(): string {
		return convertRemoteToWebUrl(this.remoteUrl);
	}

	commitUrl(commitId: string): string | undefined {
		// Different Git providers use different paths for the commit url:
		if (this.isBitBucket) {
			return `${this.pushRepoBaseUrl}/commits/${commitId}`;
		}
		if (this.isGitlab) {
			return `${this.pushRepoBaseUrl}/-/commit/${commitId}`;
		}
		return `${this.pushRepoBaseUrl}/commit/${commitId}`;
	}

	get shortName() {
		return this.branchName.split('/').slice(-1)[0];
	}

	branchUrl(upstreamBranchName: string | undefined) {
		if (!upstreamBranchName) return undefined;
		const baseBranchName = this.branchName.split('/')[1];
		const branchName = upstreamBranchName.split('/').slice(3).join('/');

		if (this.pushRemoteName) {
			if (this.isGitHub) {
				// master...schacon:docs:Virtual-branch
				const pushUsername = this.extractUsernameFromGitHubUrl(this.pushRemoteUrl);
				const pushRepoName = this.extractRepoNameFromGitHubUrl(this.pushRemoteUrl);
				return `${this.repoBaseUrl}/compare/${baseBranchName}...${pushUsername}:${pushRepoName}:${branchName}`;
			}
		}

		if (this.isBitBucket) {
			return `${this.repoBaseUrl}/branch/${branchName}?dest=${baseBranchName}`;
		}
		// The following branch path is good for at least Gitlab and Github:
		return `${this.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
	}

	private extractUsernameFromGitHubUrl(url: string): string | null {
		const regex = /github\.com[/:]([a-zA-Z0-9_-]+)\/.*/;
		const match = url.match(regex);
		return match ? match[1] : null;
	}

	private extractRepoNameFromGitHubUrl(url: string): string | null {
		const regex = /github\.com[/:]([a-zA-Z0-9_-]+)\/([a-zA-Z0-9_-]+)/;
		const match = url.match(regex);
		return match ? match[2] : null;
	}

	private get isGitHub(): boolean {
		return this.repoBaseUrl.includes('github.com');
	}

	private get isBitBucket(): boolean {
		return this.repoBaseUrl.includes('bitbucket.org');
	}

	private get isGitlab(): boolean {
		return this.repoBaseUrl.includes('gitlab.com');
	}
}
