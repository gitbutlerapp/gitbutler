import { buildContextStore } from '@gitbutler/shared/context';
import type { Octokit } from '@octokit/rest';

export class GitHubUserService {
	constructor(private octokit: Octokit) {}

	async fetchGitHubLogin(): Promise<string> {
		try {
			const rsp = await this.octokit.users.getAuthenticated();
			return rsp.data.login;
		} catch (e) {
			console.error(e);
			throw e;
		}
	}
}

export const [getGitHubUserServiceStore, createGitHubUserServiceStore] = buildContextStore<
	GitHubUserService | undefined
>('githubUserService');
