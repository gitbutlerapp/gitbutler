import { GitHub } from '$lib/forge/github/github';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach } from 'vitest';
import type { ForgePrService as GitHubPrService } from '$lib/forge/interface/forgePrService';
import type { BackendApi } from '$lib/state/clientState.svelte';

// TODO: Rewrite this proof-of-concept into something valuable.
describe('GitHubPrService', () => {
	let gh: GitHub;
	let service: GitHubPrService | undefined;

	const { gitHubClient, gitHubApi, octokit } = setupMockGitHubApi();

	beforeEach(() => {
		const MockBackendApi = vi.fn();
		MockBackendApi.prototype.injectEndpoints = vi.fn();
		const backendApi: BackendApi = new MockBackendApi();

		gh = new GitHub({
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'main',
			backendApi,
			api: gitHubApi,
			authenticated: true,
			isLoading: false,
			client: gitHubClient
		});
		service = gh.prService;
	});

	test('test parsing response', async () => {
		const title = 'PR Title';
		vi.spyOn(octokit.pulls, 'get').mockReturnValue(
			Promise.resolve({
				data: { title, updated_at: new Date().toISOString() }
			} as RestEndpointMethodTypes['pulls']['get']['response'])
		);
		const pr = await service?.fetch(123);
		expect(pr?.title).equal(title);
	});
});
