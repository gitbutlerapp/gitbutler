import { GitHub } from './github';
import { GitHubPrMonitor, PR_SERVICE_INTERVAL } from './githubPrMonitor.svelte';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { ForgePrMonitor } from '../interface/forgePrMonitor';
import type { ForgePrService } from '../interface/forgePrService';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubPrMonitor', () => {
	let gh: GitHub;
	let service: ForgePrService | undefined;
	let monitor: ForgePrMonitor | undefined;

	const { gitHubApi, octokit } = setupMockGitHubApi();

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
			gitHubApi
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
