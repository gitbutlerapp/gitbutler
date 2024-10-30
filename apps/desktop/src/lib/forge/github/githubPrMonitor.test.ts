import { GitHub } from './github';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { ForgePrMonitor } from '../interface/forgePrMonitor';
import type { ForgePrService } from '../interface/forgePrService';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubPrMonitor', () => {
	let octokit: Octokit;
	let gh: GitHub;
	let service: ForgePrService | undefined;
	let monitor: ForgePrMonitor | undefined;

	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	beforeEach(() => {
		octokit = new Octokit();
		gh = new GitHub({
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'test-branch',
			octokit
		});
		service = gh.prService();
		monitor = service?.prMonitor(123);
	});

	test('should run on set interval', async () => {
		const get = vi.spyOn(octokit.pulls, 'get').mockReturnValue(
			Promise.resolve({
				data: { title: 'PR Title' }
			} as RestEndpointMethodTypes['pulls']['get']['response'])
		);
		const unsubscribe = monitor?.pr.subscribe(() => {});
		expect(get).toHaveBeenCalledOnce();
		vi.advanceTimersToNextTimer();
		expect(get).toHaveBeenCalledTimes(2);

		// Unsubscribing should cancel the interval.
		unsubscribe?.();
		vi.advanceTimersToNextTimer();
		expect(get).toHaveBeenCalledTimes(2);
	});
});
