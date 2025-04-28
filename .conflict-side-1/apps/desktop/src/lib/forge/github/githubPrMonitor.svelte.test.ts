import { GitHub } from '$lib/forge/github/github';
import { GitHubPrMonitor, PR_SERVICE_INTERVAL } from '$lib/forge/github/githubPrMonitor.svelte';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { ForgePrMonitor } from '$lib/forge/interface/forgePrMonitor';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubPrMonitor', () => {
	let gh: GitHub;
	let service: ForgePrService | undefined;
	let monitor: ForgePrMonitor | undefined;

	const { gitHubClient, gitHubApi, octokit } = setupMockGitHubApi();

	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	beforeEach(() => {
		gh = new GitHub({
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'test-branch',
			api: gitHubApi,
			client: gitHubClient,
			authenticated: true
		});
		service = gh.prService;
		monitor = new GitHubPrMonitor(service, 123);
	});

	test('should run on set interval', async () => {
		const get = vi.spyOn(octokit.pulls, 'get').mockReturnValue(
			Promise.resolve({
				data: { title: 'PR Title' }
			} as RestEndpointMethodTypes['pulls']['get']['response'])
		);

		const unsubscribe = monitor?.pr.subscribe(() => {});
		expect(get).toHaveBeenCalledOnce();
		await vi.advanceTimersByTimeAsync(PR_SERVICE_INTERVAL);
		expect(get).toHaveBeenCalledTimes(2);

		// Unsubscribing should cancel the interval.
		unsubscribe?.();
		await vi.advanceTimersByTimeAsync(PR_SERVICE_INTERVAL);
		expect(get).toHaveBeenCalledTimes(2);
	});
});
