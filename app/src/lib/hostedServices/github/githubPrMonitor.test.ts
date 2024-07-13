import { GitHubService } from './githubService';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { HostedGitPrMonitor } from '../interface/hostedGitPrMonitor';
import type { HostedGitPrService } from '../interface/hostedGitPrService';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubPrMonitor', () => {
	let octokit: Octokit;
	let gh: GitHubService;
	let service: HostedGitPrService;
	let monitor: HostedGitPrMonitor;

	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	beforeEach(() => {
		octokit = new Octokit();
		gh = new GitHubService(new ProjectMetrics(), octokit, {
			provider: 'github.com',
			name: 'test-repo',
			owner: 'test-owner'
		});
		service = gh.prService('base-branch', 'upstream-branch');
		monitor = service.prMonitor(123);
	});

	test('should run on set interval', async () => {
		const mock = vi.spyOn(octokit.pulls, 'get').mockReturnValue(
			Promise.resolve({
				data: { title: 'PR Title' }
			} as RestEndpointMethodTypes['pulls']['get']['response'])
		);
		const unsubscribe = monitor.pr.subscribe(() => {});
		expect(mock).toHaveBeenCalledOnce();
		vi.advanceTimersToNextTimer();
		expect(mock).toHaveBeenCalledTimes(2);

		// Unsubscribing should cancel the interval.
		unsubscribe();
		vi.advanceTimersToNextTimer();
		expect(mock).toHaveBeenCalledTimes(2);
	});
});
