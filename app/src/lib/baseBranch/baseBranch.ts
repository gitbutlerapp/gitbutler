import { convertRemoteToWebUrl } from '$lib/utils/url';
import { RemoteCommit } from '$lib/vbranches/types';
import { Type } from 'class-transformer';

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
