import { DefaultForgeFactory } from './forgeFactory';
import { GitHub } from './github/github';
import { GitLab } from './gitlab/gitlab';
import { Octokit } from '@octokit/rest';
import { expect, test, describe } from 'vitest';

describe.concurrent('DefaultforgeFactory', () => {
	test('Create GitHub service', async () => {
		const monitorFactory = new DefaultForgeFactory(new Octokit());
		expect(
			monitorFactory.build(
				{
					domain: 'github.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				'some-base'
			)
		).instanceOf(GitHub);
	});

	test('Create self hosted Gitlab service', async () => {
		const monitorFactory = new DefaultForgeFactory(new Octokit());
		expect(
			monitorFactory.build(
				{
					domain: 'gitlab.domain.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				'some-base'
			)
		).instanceOf(GitLab);
	});

	test('Create Gitlab service', async () => {
		const monitorFactory = new DefaultForgeFactory(new Octokit());
		expect(
			monitorFactory.build(
				{
					domain: 'gitlab.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				'some-base'
			)
		).instanceOf(GitLab);
	});
});
