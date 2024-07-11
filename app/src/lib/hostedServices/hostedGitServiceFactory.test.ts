import { GitHubService } from './github/githubService';
import { DefaultHostedGitServiceFactory } from './hostedGitServiceFactory';
import { Octokit } from '@octokit/rest';
import { expect, test, describe } from 'vitest';

describe.concurrent('DefaultHostedGitServiceFactory', () => {
	test('Create GitHub service', async () => {
		const monitorFactory = new DefaultHostedGitServiceFactory(new Octokit());
		expect(
			monitorFactory.build({
				provider: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			})
		).instanceOf(GitHubService);
	});
});
