import { GitHub } from '$lib/forge/github/github';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { expect, test, describe, vi } from 'vitest';
import type { BackendApi } from '$lib/state/clientState.svelte';

describe('GitHub', () => {
	const { gitHubApi, gitHubClient } = setupMockGitHubApi();

	const MockBackendApi = vi.fn();
	MockBackendApi.prototype.injectEndpoints = vi.fn();
	const backendApi: BackendApi = new MockBackendApi();

	const id = 'some-branch';
	const repo = {
		domain: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	test('commit url', async () => {
		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			backendApi,
			repo,
			baseBranch: id,
			authenticated: true,
			isLoading: false
		});
		const url = gh.commitUrl(id);
		expect(url).toMatch(new RegExp(`/${id}$`));
	});

	test('uses https protocol for ssh remote urls (browser compatibility)', () => {
		const sshRepo = {
			...repo,
			protocol: 'ssh'
		};

		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			backendApi,
			repo: sshRepo,
			baseBranch: id,
			authenticated: true,
			isLoading: false
		});

		expect(gh.commitUrl('abc123')).toBe('https://github.com/test-owner/test-repo/commit/abc123');
	});

	test('branch urls use https protocol for ssh remote urls', () => {
		const sshRepo = {
			...repo,
			protocol: 'ssh'
		};

		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			backendApi,
			repo: sshRepo,
			baseBranch: 'main',
			authenticated: true,
			isLoading: false
		});

		const branch = gh.branch('feature-branch');
		expect(branch?.url).toBe(
			'https://github.com/test-owner/test-repo/compare/main...feature-branch'
		);
	});

	test('handles ssh protocol with colon suffix', () => {
		const sshRepo = {
			...repo,
			protocol: 'ssh:'
		};

		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			backendApi,
			repo: sshRepo,
			baseBranch: id,
			authenticated: true,
			isLoading: false
		});

		expect(gh.commitUrl('abc123')).toBe('https://github.com/test-owner/test-repo/commit/abc123');
	});

	test('uses https protocol for ssh remote urls on GitHub Enterprise', () => {
		const enterpriseRepo = {
			domain: 'github.mycompany.com',
			name: 'test-repo',
			owner: 'test-owner',
			protocol: 'ssh'
		};

		const gh = new GitHub({
			api: gitHubApi,
			client: gitHubClient,
			backendApi,
			repo: enterpriseRepo,
			baseBranch: id,
			authenticated: true,
			isLoading: false
		});

		expect(gh.commitUrl('abc123')).toBe(
			'https://github.mycompany.com/test-owner/test-repo/commit/abc123'
		);
	});
});
