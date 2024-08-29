import { DefaultGitHostFactory } from './gitHostFactory';
import { GitHub } from './github/github';
import { GitLab } from './gitlab/gitlab';
import { Octokit } from '@octokit/rest';
import { expect, test, describe } from 'vitest';

describe.concurrent('DefaultgitHostFactory', () => {
	test('Create GitHub service', async () => {
		const monitorFactory = new DefaultGitHostFactory(new Octokit());
		expect(
			monitorFactory.build(
				{
					source: 'github.com',
					name: 'test-repo',
					owner: 'test-owner',
					resource: 'test-resource'
				},
				'some-base'
			)
		).instanceOf(GitHub);
	});

	test('Create self hosted Gitlab service', async () => {
		const monitorFactory = new DefaultGitHostFactory(new Octokit());
		expect(
			monitorFactory.build(
				{
					source: 'domain.com',
					name: 'test-repo',
					owner: 'test-owner',
					resource: 'gitlab.domain.com'
				},
				'some-base'
			)
		).instanceOf(GitLab);
	});

	test('Create Gitlab service', async () => {
		const monitorFactory = new DefaultGitHostFactory(new Octokit());
		expect(
			monitorFactory.build(
				{
					source: 'gitlab.com',
					name: 'test-repo',
					owner: 'test-owner',
					resource: 'test-resource'
				},
				'some-base'
			)
		).instanceOf(GitLab);
	});
});
