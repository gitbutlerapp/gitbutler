import { GitHub } from './github';
import { MIN_COMPLETED_AGE } from './githubChecksMonitor';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { get } from 'svelte/store';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { GitHostChecksMonitor } from '../interface/forgeChecksMonitor';

type ChecksResponse = RestEndpointMethodTypes['checks']['listForRef']['response'];
type CheckRuns = RestEndpointMethodTypes['checks']['listForRef']['response']['data']['check_runs'];

type SuitesResponse = RestEndpointMethodTypes['checks']['listSuitesForRef']['response'];
type CheckSuites =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data']['check_suites'];

// TODO: Rewrite this proof-of-concept into something valuable.
describe('GitHubChecksMonitor', () => {
	let octokit: Octokit;
	let gh: GitHub;
	let monitor: GitHostChecksMonitor | undefined;

	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
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
		monitor = gh.checksMonitor('upstream-branch');
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

		await monitor?.update();
		expect(listForRef).toHaveBeenCalledOnce();
		expect(listSuitesForRef).toHaveBeenCalledOnce();

		const checks = monitor ? get(monitor?.status) : undefined;
		expect(checks).toBeNull();
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
		await monitor?.update();
		expect(mock).toHaveBeenCalledOnce();

		let status = monitor?.getLastStatus();
		expect(status?.finished).toBeFalsy();

		// Verify that checks are re-fetchd after some timeout.
		await vi.runOnlyPendingTimersAsync();
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
		await vi.runOnlyPendingTimersAsync();
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
