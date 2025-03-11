import { GitHub } from './github';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { test, describe, vi, beforeEach, afterEach, expect } from 'vitest';
import type { ForgeListingService } from '../interface/forgeListingService';

type Labels = RestEndpointMethodTypes['pulls']['list']['response']['data'][0]['labels'];
type PrListResponse = RestEndpointMethodTypes['pulls']['list']['response'];

describe.concurrent('GitHubListingService', () => {
	const repoInfo = {
		domain: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	let octokit: Octokit;
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
		octokit = new Octokit();
		projectMetrics = new ProjectMetrics('test-project');

		gh = new GitHub({ repo: repoInfo, baseBranch: 'some-base', octokit, projectMetrics, posthog });
		service = gh.listService();
	});

	test('should update project metrics', async () => {
		const title = 'PR Title';
		vi.spyOn(octokit.pulls, 'list').mockReturnValue(
			Promise.resolve({
				data: [{ title, labels: [] as Labels }]
			} as PrListResponse)
		);
		await service?.refresh();
		const metrics = projectMetrics.getReport();
		expect(metrics['pr_count']?.value).toEqual(1);
		expect(metrics['pr_count']?.maxValue).toEqual(1);
		expect(metrics['pr_count']?.minValue).toEqual(1);
	});
});
