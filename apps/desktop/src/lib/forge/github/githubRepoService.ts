import { readable, type Readable } from 'svelte/store';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { ForgeRepoService, RepoDetailedInfo } from '../interface/forgeRepoService';
import type { Octokit } from '@octokit/rest';

export class GitHubRepoService implements ForgeRepoService {
	info: Readable<RepoDetailedInfo | undefined>;

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo
	) {
		this.info = readable<RepoDetailedInfo | undefined>(undefined, (set) => {
			this.getInfo().then((info) => {
				set(info);
			});
		});
	}

	private async getInfo(): Promise<RepoDetailedInfo> {
		const response = await this.octokit.rest.repos.get({
			owner: this.repo.owner,
			repo: this.repo.name
		});

		return {
			deleteBranchAfterMerge: response.data.delete_branch_on_merge
		};
	}
}
