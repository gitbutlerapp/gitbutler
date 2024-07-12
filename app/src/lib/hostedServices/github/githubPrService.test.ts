import { GitHubService } from './githubService';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach } from 'vitest';
import type { HostedGitPrService } from '../interface/hostedGitPrService';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubPrService', () => {
	let octokit: Octokit;
	let gh: GitHubService;
	let service: HostedGitPrService;

	beforeEach(() => {
		octokit = new Octokit();
		gh = new GitHubService(new ProjectMetrics(), octokit, {
			provider: 'github.com',
			name: 'test-repo',
			owner: 'test-owner'
		});
		service = gh.prService('base-branch', 'upstream-branch');
	});

	test('Test parsing response', async () => {
		const title = 'PR Title';
		vi.spyOn(octokit.pulls, 'get').mockReturnValueOnce(
			Promise.resolve({
				data: { title }
			} as RestEndpointMethodTypes['pulls']['get']['response'])
		);
		const pr = await service.get(123);
		expect(pr?.title).equal(title);
	});
});
