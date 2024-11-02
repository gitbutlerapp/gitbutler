import { GitHub } from './github';
import { ForgeName } from '$lib/forge/interface/types';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { expect, test, describe, vi, beforeEach } from 'vitest';
import type { ForgePrService as GitHubPrService } from '../interface/forgePrService';

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
		const pr = await service?.get({ type: ForgeName.GitHub, subject: { prNumber: 123 } });
		expect(pr?.title).equal(title);
	});
});
