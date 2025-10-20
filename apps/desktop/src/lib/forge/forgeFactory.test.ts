import { EventContext } from '$lib/analytics/eventContext';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import { GitHub } from '$lib/forge/github/github';
import { GitLab } from '$lib/forge/gitlab/gitlab';
import { type BackendApi, type GitHubApi } from '$lib/state/clientState.svelte';
import { mockCreateBackend } from '$lib/testing/mockBackend';
import { getSettingsdServiceMock } from '$lib/testing/mockSettingsdService';
import { expect, test, describe, vi } from 'vitest';
import type { GitHubClient } from '$lib/forge/github/githubClient';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';

describe.concurrent('DefaultforgeFactory', () => {
	const MockSettingsService = getSettingsdServiceMock();
	const backend = mockCreateBackend();
	const settingsService = new MockSettingsService();
	const eventContext = new EventContext();
	const posthog = new PostHogWrapper(settingsService, backend, eventContext);
	const gitHubApi: GitHubApi = {
		endpoints: {},
		reducerPath: 'github',
		internalActions: undefined as any,
		util: undefined as any,
		reducer: undefined as any,
		middleware: undefined as any,
		injectEndpoints: vi.fn(),
		enhanceEndpoints: undefined as any
	};
	const MockBackendApi = vi.fn();
	MockBackendApi.prototype.injectEndpoints = vi.fn();
	const backendApi: BackendApi = new MockBackendApi();
	const gitHubClient = { onReset: () => {} } as any as GitHubClient;
	const gitLabClient = { onReset: () => {} } as any as GitLabClient;

	// TODO: Replace with a better mock.
	const dispatch = (() => {}) as ThunkDispatch<any, any, UnknownAction>;
	const gitLabApi: any = {
		injectEndpoints: vi.fn()
	};

	test('Create GitHub service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			backendApi,
			gitLabClient,
			gitLabApi,
			posthog,
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
				detectedForgeProvider: undefined,
				forgeOverride: undefined
			})
		).instanceOf(GitHub);
	});

	test('Create self hosted Gitlab service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			backendApi,
			gitLabClient,
			gitLabApi,
			posthog,
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
				detectedForgeProvider: undefined,
				forgeOverride: undefined
			})
		).instanceOf(GitLab);
	});

	test('Create Gitlab service', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			backendApi,
			gitLabClient,
			gitLabApi,
			posthog,
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
				detectedForgeProvider: undefined,
				forgeOverride: undefined
			})
		).instanceOf(GitLab);
	});

	test('Respects detectedForgeProvider: GitHub', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			backendApi,
			gitLabClient,
			gitLabApi,
			posthog,
			dispatch
		});
		const result = factory.build({
			repo: {
				domain: 'gitlab.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'main',
			detectedForgeProvider: 'github',
			forgeOverride: undefined
		});
		expect(result).instanceOf(GitHub);
	});

	test('Respects detectedForgeProvider: GitLab', async () => {
		const factory = new DefaultForgeFactory({
			gitHubClient,
			gitHubApi,
			gitLabClient,
			backendApi,
			gitLabApi,
			posthog,
			dispatch
		});
		const result = factory.build({
			repo: {
				domain: 'github.com',
				name: 'test-repo',
				owner: 'test-owner'
			},
			baseBranch: 'main',
			detectedForgeProvider: 'gitlab',
			forgeOverride: undefined
		});
		expect(result).instanceOf(GitLab);
	});
});
