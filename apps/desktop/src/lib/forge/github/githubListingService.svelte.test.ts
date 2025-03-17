import { GitHub } from './github';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { type RestEndpointMethodTypes } from '@octokit/rest';
import { flushSync } from 'svelte';
import { test, describe, vi, beforeEach, afterEach, expect } from 'vitest';
import type { ForgeListingService } from '../interface/forgeListingService';

type Labels = RestEndpointMethodTypes['pulls']['list']['response']['data'][0]['labels'];
type PrListResponse = RestEndpointMethodTypes['pulls']['list']['response'];

const PROJECT_ID = 'some-project';

describe('GitHubListingService', () => {
	const repoInfo = {
		domain: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	const { gitHubApi, octokit } = setupMockGitHubApi();

	let gh: GitHub;
	let service: ForgeListingService | undefined;
	let projectMetrics: ProjectMetrics;
	const posthog = new PostHogWrapper();

	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	beforeEach(() => {
		projectMetrics = new ProjectMetrics();
		gh = new GitHub({
			gitHubApi,
			repo: repoInfo,
			baseBranch: 'some-base',
			projectMetrics,
			posthog
		});
		service = gh.listService;
	});

	test('should update project metrics', async () => {
		const title = 'PR Title';
		vi.spyOn(octokit.pulls, 'list').mockReturnValue(
			Promise.resolve({
				data: [{ title, labels: [] as Labels }]
			} as PrListResponse)
		);

		// Listing has the effect of logging pr count.
		const cleanup = $effect.root(() => {
			service?.list(PROJECT_ID);
			flushSync();
		});

		// TODO: Why do we need to advance these timers in addition to `flushSync`?
		await vi.advanceTimersToNextTimerAsync();
		cleanup();

		const metrics = projectMetrics.getReport(PROJECT_ID);
		expect(metrics['pr_count']?.value).toEqual(1);
		expect(metrics['pr_count']?.maxValue).toEqual(1);
		expect(metrics['pr_count']?.minValue).toEqual(1);
	});
});
