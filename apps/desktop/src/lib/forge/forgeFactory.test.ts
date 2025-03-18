import { DefaultForgeFactory } from './forgeFactory.svelte';
import { GitHub } from './github/github';
import { GitLab } from './gitlab/gitlab';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { type GitHubApi } from '$lib/state/clientState.svelte';
import { expect, test, describe } from 'vitest';

describe.concurrent('DefaultforgeFactory', () => {
	const posthog = new PostHogWrapper();
	const projectMetrics = new ProjectMetrics();
	const githubApi: GitHubApi = {
		endpoints: {},
		reducerPath: 'github',
		internalActions: undefined as any,
		util: undefined as any,
		reducer: undefined as any,
		middleware: undefined as any,
		injectEndpoints: undefined as any,
		enhanceEndpoints: undefined as any
	};
	test('Create GitHub service', async () => {
		const factory = new DefaultForgeFactory(githubApi, posthog, projectMetrics);
		expect(
			factory.build({
				repo: {
					domain: 'github.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base'
			})
		).instanceOf(GitHub);
	});

	test('Create self hosted Gitlab service', async () => {
		const factory = new DefaultForgeFactory(githubApi, posthog, projectMetrics);
		expect(
			factory.build({
				repo: {
					domain: 'gitlab.domain.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base'
			})
		).instanceOf(GitLab);
	});

	test('Create Gitlab service', async () => {
		const factory = new DefaultForgeFactory(githubApi, posthog, projectMetrics);
		expect(
			factory.build({
				repo: {
					domain: 'gitlab.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base'
			})
		).instanceOf(GitLab);
	});
});
