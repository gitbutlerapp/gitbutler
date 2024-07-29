import { DefaultGitHostFactory } from './gitHostFactory';
import { GitHub } from './github/github';
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
					owner: 'test-owner'
				},
				'some-base'
			)
		).instanceOf(GitHub);
	});
});
