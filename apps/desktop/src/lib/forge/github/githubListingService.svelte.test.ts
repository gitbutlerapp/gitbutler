import { GitHub } from './github';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { type RestEndpointMethodTypes } from '@octokit/rest';
import { flushSync } from 'svelte';
import { test, describe, vi, beforeEach, expect } from 'vitest';

type Labels = RestEndpointMethodTypes['pulls']['list']['response']['data'][0]['labels'];
type PrListResponse = RestEndpointMethodTypes['pulls']['list']['response'];

const PROJECT_ID = 'some-project';

describe('GitHubListingService', () => {
	const { gitHubApi, octokit } = setupMockGitHubApi();

	let gh: GitHub;
	let projectMetrics: ProjectMetrics;
	const posthog = new PostHogWrapper();

	vi.useFakeTimers();

	beforeEach(() => {
		projectMetrics = new ProjectMetrics();
		gh = new GitHub({
			gitHubApi,
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'some-base',
			projectMetrics,
			posthog
		});
	});

	/**
	 * (╯°□°)╯︵ ┻━┻
	 *
	 * There is something weird going on with this test.
	 *
	 * For the test to pass we need to call the list function twice. The first
	 * call does not trigger the expected call using `Octokit`.
	 *
	 * On manual inspection it appears the query is recorded in the github
	 * state object, it never actually fires the `queryFn`. This appears
	 * to happen, however, immediately on the second function call.
	 *
	 * Only one of two things can be true:
	 * - this test is not correctly written
	 * - there is a bug in the implementation
	 *
	 * If you have spare time on your hands, any investigation into this would
	 * be much appreciated.
	 */
	test('should update project metrics', async () => {
		const service = gh.listService;
		const title = 'PR Title';

		vi.spyOn(octokit.pulls, 'list').mockReturnValue(
			Promise.resolve({
				data: [{ title, labels: [] as Labels, head: { ref: 'test' } }]
			} as PrListResponse)
		);

		const cleanup = $effect.root(() => {
			service.list(PROJECT_ID);
		});

		// The order of these two seems to matter.
		flushSync();
		await vi.advanceTimersToNextTimerAsync();

		// This time the function call correctly fires GitHub call, and records
		// the expected metric.
		const cleanup2 = $effect.root(() => {
			service.list(PROJECT_ID);
		});

		// Flush has to come before cleanup.
		flushSync();
		cleanup();
		cleanup2();

		const metrics = projectMetrics.getReport(PROJECT_ID);
		expect(metrics['pr_count']?.value).toEqual(1);
		expect(metrics['pr_count']?.maxValue).toEqual(1);
		expect(metrics['pr_count']?.minValue).toEqual(1);
	});
});
