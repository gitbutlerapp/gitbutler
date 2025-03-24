import { GitHub } from '$lib/forge/github/github';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { expect, test, describe } from 'vitest';

describe('GitHub', () => {
	const { gitHubApi } = setupMockGitHubApi();

	const id = 'some-branch';
	const repo = {
		domain: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	test('commit url', async () => {
		const gh = new GitHub({ repo, baseBranch: id, gitHubApi, authenticated: true });
		const url = gh.commitUrl(id);
		expect(url).toMatch(new RegExp(`/${id}$`));
	});
});
