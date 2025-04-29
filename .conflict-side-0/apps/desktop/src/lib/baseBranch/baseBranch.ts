import { Commit } from '$lib/commits/commit';
import { Type } from 'class-transformer';

export interface RemoteBranchInfo {
	name: string;
}

export class NoDefaultTarget extends Error {}

export class BaseBranch {
	branchName!: string;
	remoteName!: string;
	remoteUrl!: string;
	pushRemoteName!: string;
	pushRemoteUrl!: string;
	baseSha!: string;
	currentSha!: string;
	behind!: number;
	@Type(() => Commit)
	upstreamCommits!: Commit[];
	@Type(() => Commit)
	recentCommits!: Commit[];
	lastFetchedMs?: number;
	conflicted!: boolean;
	diverged!: boolean;
	divergedAhead!: string[];
	divergedBehind!: string[];

	actualPushRemoteName(): string {
		return this.pushRemoteName || this.remoteName;
	}

	get lastFetched(): Date | undefined {
		return this.lastFetchedMs ? new Date(this.lastFetchedMs) : undefined;
	}

	get shortName() {
		return this.branchName.split('/').slice(-1)[0]!;
	}
}
