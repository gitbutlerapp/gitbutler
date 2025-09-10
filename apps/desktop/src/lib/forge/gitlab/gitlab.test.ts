import { GitLab } from '$lib/forge/gitlab/gitlab';
import { expect, test, describe, vi } from 'vitest';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { GitLabApi } from '$lib/state/clientState.svelte';

describe('GitLab', () => {
	// Mock GitLab API and client
	const gitLabApi: GitLabApi = {
		endpoints: {},
		reducerPath: 'gitlab',
		internalActions: undefined as any,
		util: undefined as any,
		reducer: undefined as any,
		middleware: undefined as any,
		injectEndpoints: vi.fn(),
		enhanceEndpoints: undefined as any
	};
	const gitLabClient = { onReset: () => {} } as any as GitLabClient;

	const baseBranch = 'main';
	const baseRepo = {
		domain: 'gitlab.example.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	test('uses https protocol by default when no protocol specified', () => {
		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo: baseRepo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'https://gitlab.example.com/test-owner/test-repo/-/commit/abc123'
		);
	});

	test('respects http protocol when specified', () => {
		const repo = {
			...baseRepo,
			protocol: 'http'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'http://gitlab.example.com/test-owner/test-repo/-/commit/abc123'
		);
	});

	test('respects https protocol when explicitly specified', () => {
		const repo = {
			...baseRepo,
			protocol: 'https'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'https://gitlab.example.com/test-owner/test-repo/-/commit/abc123'
		);
	});

	test('handles protocol with colon suffix', () => {
		const repo = {
			...baseRepo,
			protocol: 'http:'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'http://gitlab.example.com/test-owner/test-repo/-/commit/abc123'
		);
	});

	test('branch urls use correct protocol', () => {
		const repo = {
			...baseRepo,
			protocol: 'http'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		const branch = gitlab.branch('feature-branch');
		expect(branch?.url).toBe(
			'http://gitlab.example.com/test-owner/test-repo/-/compare/main...feature-branch'
		);
	});

	test('uses https protocol for ssh remote urls (browser compatibility)', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'https://gitlab.example.com/test-owner/test-repo/-/commit/abc123'
		);
	});

	test('branch urls use https protocol for ssh remote urls', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		const branch = gitlab.branch('feature-branch');
		expect(branch?.url).toBe(
			'https://gitlab.example.com/test-owner/test-repo/-/compare/main...feature-branch'
		);
	});

	test('handles ssh protocol with colon suffix', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh:'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'https://gitlab.example.com/test-owner/test-repo/-/commit/abc123'
		);
	});

	test('uses https protocol for ssh remote urls on custom GitLab instance', () => {
		const repo = {
			domain: 'gitlab.mycompany.com',
			name: 'test-repo',
			owner: 'test-owner',
			protocol: 'ssh'
		};

		const gitlab = new GitLab({
			api: gitLabApi,
			client: gitLabClient,
			repo,
			baseBranch,
			authenticated: true
		});

		expect(gitlab.commitUrl('abc123')).toBe(
			'https://gitlab.mycompany.com/test-owner/test-repo/-/commit/abc123'
		);
	});
});
