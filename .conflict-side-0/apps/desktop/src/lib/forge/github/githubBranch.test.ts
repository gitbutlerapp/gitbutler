import { GitHub } from '$lib/forge/github/github';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { expect, test, describe } from 'vitest';

// TODO: Rewrite this proof-of-concept into something valuable.
describe('GitHubBranch', () => {
	const { gitHubClient, gitHubApi } = setupMockGitHubApi();

	const name = 'some-branch';
	const baseBranch = 'some-base';
	const repo = {
		domain: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	test('branch compare url', async () => {
		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			repo,
			baseBranch,
			authenticated: true,
			isLoading: false
		});
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${name}$`));
	});

	test('fork compare url', async () => {
		const forkStr = `${repo.owner}:${repo.name}`;
		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			repo,
			baseBranch,
			forkStr,
			authenticated: true,
			isLoading: false
		});
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${forkStr}:${name}$`));
	});
});
