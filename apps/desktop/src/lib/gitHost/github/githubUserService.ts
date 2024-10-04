import { buildContextStore } from '$lib/utils/context';
import { writable } from 'svelte/store';
import type { Octokit, RestEndpointMethodTypes } from '@octokit/rest';

type GitHubGetAuthenticatedUserData =
	RestEndpointMethodTypes['users']['getAuthenticated']['response']['data'];

export class GitHubUserService {
	readonly authenticatedUser = writable<GitHubGetAuthenticatedUserData | undefined>(undefined);

	constructor(private octokit: Octokit) {}

	async fetch(): Promise<GitHubGetAuthenticatedUserData> {
		try {
			const rsp = await this.octokit.users.getAuthenticated();
			this.authenticatedUser.set(rsp.data);
			return rsp.data;
		} catch (e) {
			console.error(e);
			throw e;
		}
	}
}

export const [getGitHubUserServiceStore, createGitHubUserServiceStore] = buildContextStore<
	GitHubUserService | undefined
>('githubUserService');
