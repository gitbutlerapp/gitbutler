import { buildContextStore } from '$lib/utils/context';
import { writable } from 'svelte/store';
import type { GitHubGetAuthenticatedUserData } from './types';
import type { Octokit } from '@octokit/rest';

export class GitHubUserService {
	readonly authenticatedUser = writable<GitHubGetAuthenticatedUserData | undefined>(undefined);
	readonly userMap = writable<Record<string, GitHubGetAuthenticatedUserData>>({});

	constructor(private octokit: Octokit) {}

	async fetch(): Promise<GitHubGetAuthenticatedUserData> {
		try {
			const rsp = await this.octokit.users.getAuthenticated();
			this.authenticatedUser.set(rsp.data);
			this.userMap.update((map) => ({ ...map, [rsp.data.login]: rsp.data }));
			return rsp.data;
		} catch (e) {
			console.error(e);
			throw e;
		}
	}

	async getUserInfo(username: string) {
		const rsp = await this.octokit.users.getByUsername({ username });
		this.userMap.update((map) => ({ ...map, [rsp.data.login]: rsp.data }));
	}
}

export const [getGitHubUserServiceStore, createGitHubUserServiceStore] = buildContextStore<
	GitHubUserService | undefined
>('githubUserService');
