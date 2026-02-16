import { GitHub } from '$lib/forge/github/github';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { expect, test, describe, vi } from 'vitest';
import type { BackendApi } from '$lib/state/clientState.svelte';

// TODO: Rewrite this proof-of-concept into something valuable.
describe('GitHubBranch', () => {
	const { gitHubClient, gitHubApi } = setupMockGitHubApi();

	const MockBackendApi = vi.fn();
	MockBackendApi.prototype.injectEndpoints = vi.fn();
	const backendApi: BackendApi = new MockBackendApi();

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
			backendApi,
			repo,
			baseBranch,
			authenticated: true,
			isLoading: false,
			dispatch: () => {}
		});
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${name}$`));
	});

	test('fork compare url', async () => {
		const forkStr = `${repo.owner}:${repo.name}`;
		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			backendApi,
			repo,
			baseBranch,
			forkStr,
			authenticated: true,
			isLoading: false,
			dispatch: () => {}
		});
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${forkStr}:${name}$`));
	});
});
