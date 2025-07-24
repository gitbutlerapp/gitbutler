import { Commit } from '$lib/commits/commit';
import { InjectionToken } from '@gitbutler/shared/context';
import { Type } from 'class-transformer';

export interface RemoteBranchInfo {
	name: string;
}

export const BASE_BRANCH = new InjectionToken<BaseBranch>('BaseBranch');

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

	get lastPathComponent(): string {
		return this.branchName.split('/').slice(-1)[0]!;
	}

	/**
	 * Gets the branch name part, with common Git ref and remote prefixes stripped.
	 * For example:
	 * - "refs/remotes/origin/feature/foo" (with remoteName="origin") -> "feature/foo"
	 * - "origin/feature/foo" (with remoteName="origin") -> "feature/foo"
	 * - "refs/heads/feature/foo" -> "feature/foo"
	 * - "feature/foo" (local branch) -> "feature/foo"
	 */
	get shortName(): string {
		const name = this.branchName;

		// Handle the edge case where the branchName is exactly the remoteName
		if (this.remoteName && name === this.remoteName) {
			return '';
		}

		const prefixesToTry: string[] = [];

		if (this.remoteName) {
			// Order matters: check longer, more specific prefixes first.
			prefixesToTry.push(`refs/remotes/${this.remoteName}/`);
			prefixesToTry.push(`${this.remoteName}/`);
		}
		prefixesToTry.push('refs/heads/');

		for (const prefix of prefixesToTry) {
			if (name.startsWith(prefix)) {
				return name.substring(prefix.length);
			}
		}

		// If no prefix matched, assume branchName is already the desired part.
		return name;
	}
}
