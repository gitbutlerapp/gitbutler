import type { GitHostIssueService } from '$lib/forge/interface/forgeIssueService';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Octokit } from '@octokit/rest';

export class GitHubIssueService implements GitHostIssueService {
	constructor(
		private octokit: Octokit,
		private repository: RepoInfo
	) {}

	async create(title: string, body: string, labels: string[]): Promise<void> {
		await this.octokit.rest.issues.create({
			repo: this.repository.name,
			owner: this.repository.owner,
			title,
			body,
			labels
		});
	}

	async listLabels(): Promise<string[]> {
		const result = await this.octokit.paginate(this.octokit.rest.issues.listLabelsForRepo, {
			repo: this.repository.name,
			owner: this.repository.owner
		});

		return result.map((label) => label.name);
	}
}
