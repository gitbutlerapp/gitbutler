import { Code } from '$lib/error/knownErrors';
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
			client: gitHubClient,
			dispatch: () => {}
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

	test('should detect stacked PR across forks error', async () => {
		const mockError = {
			message: 'Validation Failed',
			response: {
				data: {
					message: 'Validation Failed',
					errors: [
						{
							resource: 'PullRequest',
							field: 'base',
							code: 'invalid'
						}
					]
				}
			}
		};

		vi.spyOn(octokit.pulls, 'create').mockRejectedValue(mockError);

		try {
			await service?.createPr({
				title: 'Test PR',
				body: 'Test body',
				draft: false,
				baseBranchName: 'feature-branch',
				upstreamName: 'my-branch'
			});
			expect.fail('Should have thrown an error');
		} catch (err: any) {
			expect(err.code).toBe(Code.GitHubStackedPrFork);
		}
	});

	test('should detect stacked PR error among multiple validation errors', async () => {
		const mockError = {
			message: 'Validation Failed',
			response: {
				data: {
					message: 'Validation Failed',
					errors: [
						{
							resource: 'Issue',
							field: 'title',
							code: 'missing'
						},
						{
							resource: 'PullRequest',
							field: 'base',
							code: 'invalid'
						}
					]
				}
			}
		};

		vi.spyOn(octokit.pulls, 'create').mockRejectedValue(mockError);

		try {
			await service?.createPr({
				title: 'Test PR',
				body: 'Test body',
				draft: false,
				baseBranchName: 'feature-branch',
				upstreamName: 'my-branch'
			});
			expect.fail('Should have thrown an error');
		} catch (err: any) {
			expect(err.code).toBe(Code.GitHubStackedPrFork);
		}
	});
});
