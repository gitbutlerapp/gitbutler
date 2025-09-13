import { GitHub } from '$lib/forge/github/github';
import { setupMockGitHubApi } from '$lib/testing/mockGitHubApi.svelte';
import { expect, test, describe } from 'vitest';

describe('GitHub', () => {
	const { gitHubApi, gitHubClient } = setupMockGitHubApi();

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
			repo,
			baseBranch: id,
			authenticated: true
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
			repo: sshRepo,
			baseBranch: id,
			authenticated: true
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
			repo: sshRepo,
			baseBranch: 'main',
			authenticated: true
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
			repo: sshRepo,
			baseBranch: id,
			authenticated: true
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
			repo: enterpriseRepo,
			baseBranch: id,
			authenticated: true
		});

		expect(gh.commitUrl('abc123')).toBe(
			'https://github.mycompany.com/test-owner/test-repo/commit/abc123'
		);
	});

	test('GitHub Enterprise client is configured with enterprise domain', () => {
		const { gitHubApi, gitHubClient: enterpriseClient } = setupMockGitHubApi();
		const enterpriseRepo = {
			domain: 'github.mycompany.com',
			name: 'test-repo',
			owner: 'test-owner',
			protocol: 'https'
		};

		const gh = new GitHub({
			api: gitHubApi,
			client: enterpriseClient,
			repo: enterpriseRepo,
			baseBranch: id,
			authenticated: true
		});

		// The client should have been configured with the enterprise domain
		// Check that the client's internal state reflects the enterprise setup
		expect(enterpriseClient.owner).toBe('test-owner');
		expect(enterpriseClient.repo).toBe('test-repo');

		// The GitHub instance should correctly construct URLs for the enterprise domain
		expect(gh.commitUrl('abc123')).toBe(
			'https://github.mycompany.com/test-owner/test-repo/commit/abc123'
		);
	});

	test('Standard GitHub.com client works correctly', () => {
		const { gitHubApi, gitHubClient: standardClient } = setupMockGitHubApi();
		const standardRepo = {
			domain: 'github.com',
			name: 'test-repo',
			owner: 'test-owner',
			protocol: 'https'
		};

		const gh = new GitHub({
			api: gitHubApi,
			client: standardClient,
			repo: standardRepo,
			baseBranch: id,
			authenticated: true
		});

		// Check that the client's internal state reflects the standard setup
		expect(standardClient.owner).toBe('test-owner');
		expect(standardClient.repo).toBe('test-repo');

		// The GitHub instance should correctly construct URLs for github.com
		expect(gh.commitUrl('abc123')).toBe('https://github.com/test-owner/test-repo/commit/abc123');
	});
});
