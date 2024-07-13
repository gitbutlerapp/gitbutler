import { DefaultGitHostServiceFactory } from './gitHostServiceFactory';
import { GitHubService } from './github/githubService';
import { Octokit } from '@octokit/rest';
import { expect, test, describe } from 'vitest';

describe.concurrent('DefaultGitHostServiceFactory', () => {
	test('Create GitHub service', async () => {
		const monitorFactory = new DefaultGitHostServiceFactory(new Octokit());
		expect(
			monitorFactory.build({
				provider: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			})
		).instanceOf(GitHubService);
	});
});
