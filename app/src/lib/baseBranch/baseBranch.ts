import { ipv4Regex } from '$lib/utils/url';
import { Commit, type ForgeType } from '$lib/vbranches/types';
import { Expose, Transform, Type, type TransformFnParams } from 'class-transformer';
import GitUrlParse from 'git-url-parse';

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

	forgeType: ForgeType | undefined;
	repoBaseUrl: string | undefined;
	@Expose({ name: 'pushRemoteUrl' })
	@Transform(({ value }: TransformFnParams) => (value ? GitUrlParse(value) : undefined))
	gitPushRemote!: GitUrlParse.GitUrl | undefined;
	commitBaseUrl: string | undefined;
	actualPushRemoteName: string | undefined;
	private generateCommitUrl: ((commitId: string) => string) | undefined;
	private generateBranchUrl: ((baseBranchName: string, branchName: string) => string) | undefined;

	// TODO: Move most if not all of this to Rust to send over finalized properties from get_base_branch_data
	// Make as many of the one-time business rules run once
	afterTransform(): void {
		const gitRemote = GitUrlParse(this.remoteUrl);
		const remoteUrlProtocol = ipv4Regex.test(gitRemote.resource) ? 'http' : 'https';
		this.repoBaseUrl = `${remoteUrlProtocol}://${gitRemote.resource}/${gitRemote.owner}/${gitRemote.name}`;
		this.forgeType = this.getForgeType(gitRemote.resource);

		this.actualPushRemoteName = this.pushRemoteName || this.remoteName;

		if (this.gitPushRemote) {
			const { organization, owner, name, protocol } = this.gitPushRemote;
			let { resource } = this.gitPushRemote;
			const webProtocol = ipv4Regex.test(resource) ? 'http' : 'https';

			if (protocol === 'ssh' && resource.startsWith('ssh.')) {
				resource = resource.slice(4);
			}

			if (this.forgeType === 'AzureDevOps') {
				this.commitBaseUrl = `${webProtocol}://${resource}/${organization}/${owner}/_git/${name}`;
			} else {
				this.commitBaseUrl = `${webProtocol}://${resource}/${owner}/${name}`;
			}

			// Different Git providers use different paths for the commit url:
			switch (this.forgeType) {
				case 'Bitbucket':
					this.generateCommitUrl = (commitId) => `${this.commitBaseUrl}/commits/${commitId}`;
					break;
				case 'GitLab':
					this.generateCommitUrl = (commitId) => `${this.commitBaseUrl}/-/commit/${commitId}`;
					break;
				case 'AzureDevOps':
				case 'GitHub':
				case 'Unknown':
				default:
					this.generateCommitUrl = (commitId) => `${this.commitBaseUrl}/commit/${commitId}`;
					break;
			}
		}

		if (this.gitPushRemote) {
			if (this.pushRemoteName) {
				if (this.forgeType === 'GitHub') {
					// master...schacon:docs:Virtual-branch
					const pushUsername = this.gitPushRemote.user;
					const pushRepoName = this.gitPushRemote.name;
					this.generateBranchUrl = (baseBranchName, branchName) =>
						`${this.repoBaseUrl}/compare/${baseBranchName}...${pushUsername}:${pushRepoName}:${branchName}`;
				}
			}

			if (!this.generateBranchUrl) {
				switch (this.forgeType) {
					case 'Bitbucket':
						this.generateBranchUrl = (baseBranchName, branchName) =>
							`${this.repoBaseUrl}/branch/${branchName}?dest=${baseBranchName}`;
						break;
					case 'AzureDevOps':
						this.generateBranchUrl = (baseBranchName, branchName) =>
							`${this.commitBaseUrl}/branchCompare?baseVersion=GB${baseBranchName}&targetVersion=GB${branchName}`;
						break;
					// The following branch path is good for at least Gitlab and Github:
					case 'GitHub':
					case 'GitLab':
					case 'Unknown':
					default:
						this.generateBranchUrl = (baseBranchName, branchName) =>
							`${this.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
						break;
				}
			}
		}
	}

	private getForgeType(repoBaseUrl: string): ForgeType {
		switch (true) {
			case repoBaseUrl.includes('github.com'):
				return 'GitHub';
			case repoBaseUrl.includes('gitlab.com'):
				return 'GitLab';
			case repoBaseUrl.includes('bitbucket.org'):
				return 'Bitbucket';
			case repoBaseUrl.includes('dev.azure.com'):
				return 'AzureDevOps';
			default:
				return 'Unknown';
		}
	}

	commitUrl(commitId: string): string | undefined {
		return this.generateCommitUrl ? this.generateCommitUrl(commitId) : undefined;
	}

	branchUrl(upstreamBranchName: string | undefined): string | undefined {
		if (!upstreamBranchName || !this.gitPushRemote || !this.generateBranchUrl) return undefined;
		// parameter and variable property, always calculate unless future memoization
		const baseBranchName = this.branchName.split('/')[1];
		const branchName = upstreamBranchName.split('/').slice(3).join('/');

		return this.generateBranchUrl(baseBranchName, branchName);
	}

	get lastFetched(): Date | undefined {
		return this.lastFetchedMs ? new Date(this.lastFetchedMs) : undefined;
	}

	get shortName() {
		return this.branchName.split('/').slice(-1)[0];
	}
}
