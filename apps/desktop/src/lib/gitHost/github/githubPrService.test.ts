import { GitHub } from './github';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach } from 'vitest';
import type { GitHostPrService as GitHubPrService } from '../interface/gitHostPrService';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubPrService', () => {
	let octokit: Octokit;
	let gh: GitHub;
	let service: GitHubPrService | undefined;

	beforeEach(() => {
		octokit = new Octokit();
		gh = new GitHub({
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'main',
			octokit
		});
		service = gh.prService();
	});

	test('test parsing response', async () => {
		const title = 'PR Title';
		vi.spyOn(octokit.pulls, 'get').mockReturnValueOnce(
			Promise.resolve({
				data: { title }
			} as RestEndpointMethodTypes['pulls']['get']['response'])
		);
		const pr = await service?.get(123);
		expect(pr?.title).equal(title);
	});
});
