import { PostHogWrapper } from '$lib/analytics/posthog';
import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import { GitHub } from '$lib/forge/github/github';
import { GitLab } from '$lib/forge/gitlab/gitlab';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { type GitHubApi } from '$lib/state/clientState.svelte';
import { getSettingsdServiceMock } from '$lib/testing/mockSettingsdService';
import { expect, test, describe } from 'vitest';
import type { GitHubClient } from '$lib/forge/github/githubClient';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';
import type { GiteaClient } from '$lib/forge/gitea/giteaClient.svelte';
import { Gitea } from '$lib/forge/gitea/gitea';

describe.concurrent('DefaultforgeFactory', () => {
	const MockSettingsService = getSettingsdServiceMock();
	const settingsService = new MockSettingsService();
	const posthog = new PostHogWrapper(settingsService);
	const projectMetrics = new ProjectMetrics();
	const gitHubApi: GitHubApi = {
		endpoints: {},
		reducerPath: 'github',
		internalActions: undefined as any,
		util: undefined as any,
		reducer: undefined as any,
		middleware: undefined as any,
		injectEndpoints: undefined as any,
		enhanceEndpoints: undefined as any
	};
	const gitHubClient = { onReset: () => {} } as any as GitHubClient;
	const gitLabClient = { onReset: () => {} } as any as GitLabClient;
	const giteaClient = { onReset: () => {} } as any as GiteaClient;

	// TODO: Replace with a better mock.
	const dispatch = (() => {}) as ThunkDispatch<any, any, UnknownAction>;
	const gitLabApi: any = {};
	const giteaApi: any = {};

	test('Create GitHub service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			gitLabClient,
			gitLabApi,
			giteaClient,
			giteaApi,
			posthog,
			projectMetrics,
			dispatch
		});
		expect(
			factory.build({
				repo: {
					domain: 'github.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base',
				forgeOverride: undefined
			})
		).instanceOf(GitHub);
	});

	test('Create self hosted Gitlab service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			gitLabClient,
			gitLabApi,
			giteaClient,
			giteaApi,
			posthog,
			projectMetrics,
			dispatch
		});
		expect(
			factory.build({
				repo: {
					domain: 'gitlab.domain.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base',
				forgeOverride: undefined
			})
		).instanceOf(GitLab);
	});

	test('Create Gitlab service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			gitLabClient,
			gitLabApi,
			giteaClient,
			giteaApi,
			posthog,
			projectMetrics,
			dispatch
		});
		expect(
			factory.build({
				repo: {
					domain: 'gitlab.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base',
				forgeOverride: undefined
			})
		).instanceOf(GitLab);
	});

	test('Create self hosted Gitea service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			gitLabClient,
			gitLabApi,
			giteaClient,
			giteaApi,
			posthog,
			projectMetrics,
			dispatch
		});
		expect(
			factory.build({
				repo: {
					domain: 'gitea.domain.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base',
				forgeOverride: undefined
			})
		).instanceOf(Gitea);
	});

	test('Create Gitea service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			gitLabClient,
			gitLabApi,
			giteaClient,
			giteaApi,
			posthog,
			projectMetrics,
			dispatch
		});
		expect(
			factory.build({
				repo: {
					domain: 'gitea.com',
					name: 'test-repo',
					owner: 'test-owner'
				},
				baseBranch: 'some-base',
				forgeOverride: undefined
			})
		).instanceOf(Gitea);
	});
});
