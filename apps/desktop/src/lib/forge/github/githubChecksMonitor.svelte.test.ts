import { GitHub } from './github';
import { GitHubChecksMonitor, MIN_COMPLETED_AGE } from './githubChecksMonitor.svelte';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { type RestEndpointMethodTypes } from '@octokit/rest';
import { flushSync } from 'svelte';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';

type ChecksResponse = RestEndpointMethodTypes['checks']['listForRef']['response'];
type CheckRuns = RestEndpointMethodTypes['checks']['listForRef']['response']['data']['check_runs'];

type SuitesResponse = RestEndpointMethodTypes['checks']['listSuitesForRef']['response'];
type CheckSuites =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data']['check_suites'];

describe('GitHubChecksMonitor', () => {
	let gh: GitHub;
	let monitor!: GitHubChecksMonitor;

	const { gitHubApi, octokit, resetGitHubMock } = setupMockGitHubApi();

	beforeEach(() => {
		vi.useFakeTimers();
	});

	beforeEach(() => {
		vi.clearAllMocks();
		vi.clearAllTimers();
		resetGitHubMock();

		gh = new GitHub({
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'test-branch',
			gitHubApi
		});
		monitor = gh.checksMonitor('upstream-branch');
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
		monitor.stop();
	});

	test('fetch once if no check runs  or suites', async () => {
		// Return no check runs
		const listForRef = vi.spyOn(octokit.checks, 'listForRef').mockReturnValue(
			Promise.resolve({
				data: { total_count: 0, check_runs: [] as CheckRuns }
			} as ChecksResponse)
		);
		// Return no check suites
		const listSuitesForRef = vi.spyOn(octokit.checks, 'listSuitesForRef').mockReturnValue(
			Promise.resolve({
				data: { total_count: 0, check_suites: [] as CheckSuites }
			} as SuitesResponse)
		);
		monitor.start();
		await vi.advanceTimersByTimeAsync(1000);
		flushSync();
		expect(listForRef).toHaveBeenCalledOnce();
		expect(listSuitesForRef).toHaveBeenCalledOnce();

		expect(monitor.getLastStatus()).toBeNull();
	});

	test('fetch until completed', async () => {
		const startedAt = new Date();
		vi.setSystemTime(startedAt);

		const mock = vi.spyOn(octokit.checks, 'listForRef').mockReturnValue(
			Promise.resolve({
				data: {
					total_count: 1,
					check_runs: [
						{
							id: 1,
							started_at: startedAt.toISOString()
						}
					] as CheckRuns
				}
			} as ChecksResponse)
		);
		monitor.start();

		await vi.advanceTimersToNextTimerAsync();

		expect(mock).toHaveBeenCalledOnce();
		let status = monitor?.getLastStatus();
		expect(status?.completed).toBeFalsy();

		let delay = monitor.getNextDelay();
		if (delay) await vi.advanceTimersByTimeAsync(delay);
		await vi.advanceTimersToNextTimerAsync();

		expect(mock).toHaveBeenCalledTimes(2);
		mock.mockRestore();

		// Change response to something considered completed, and reset time so
		// that a next update is scheduled. Note that the check-run age is at
		// least the "time" that elapses as we run
		// `vi.runOnlyPendingTimersAsync()`.
		vi.setSystemTime(new Date(startedAt.getTime()));
		const mock2 = vi.spyOn(octokit.checks, 'listForRef').mockReturnValue(
			Promise.resolve({
				data: {
					total_count: 1,
					check_runs: [
						{
							id: 1,
							started_at: startedAt.toISOString(),
							completed_at: new Date().toISOString()
						}
					] as CheckRuns
				}
			} as ChecksResponse)
		);

		delay = monitor.getNextDelay();
		if (delay) await vi.advanceTimersByTimeAsync(delay);

		expect(mock2).toHaveBeenCalledOnce();
		status = monitor?.getLastStatus();
		expect(status?.completed).toBeTruthy();

		// Set time to be above minimum age for polling to be stopped.
		vi.setSystemTime(new Date(startedAt.getTime() + MIN_COMPLETED_AGE));
		await vi.runOnlyPendingTimersAsync();
		expect(mock2).toHaveBeenCalledTimes(2);

		// Verify polling has stopped.
		await vi.runAllTimersAsync();
		expect(mock2).toHaveBeenCalledTimes(2);
	});
});
