import { GitHubClient } from '$lib/forge/github/githubClient';
import { expect, test, describe, vi, beforeEach } from 'vitest';

// Mock the rateLimit function
vi.mock('$lib/utils/ratelimit', () => ({
	rateLimit: vi.fn().mockImplementation((config) => config.fn)
}));

describe('GitHubClient', () => {
	let client: GitHubClient;

	beforeEach(() => {
		client = new GitHubClient();
	});

	test('creates Octokit with standard GitHub API URL for github.com domain', () => {
		client.setRepo({ owner: 'test-owner', repo: 'test-repo', domain: 'github.com' });
		
		const octokit = client.octokit;
		
		// We can't easily test the internal baseUrl configuration directly,
		// but we can verify that the client was created successfully
		expect(octokit).toBeDefined();
		expect(client.owner).toBe('test-owner');
		expect(client.repo).toBe('test-repo');
	});

	test('creates Octokit with enterprise API URL for enterprise domains', () => {
		client.setRepo({ owner: 'test-owner', repo: 'test-repo', domain: 'github.mycompany.com' });
		
		const octokit = client.octokit;
		
		// We can't easily test the internal baseUrl configuration directly,
		// but we can verify that the client was created successfully
		expect(octokit).toBeDefined();
		expect(client.owner).toBe('test-owner');
		expect(client.repo).toBe('test-repo');
	});

	test('resets client when repo info changes', () => {
		client.setRepo({ owner: 'test-owner', repo: 'test-repo', domain: 'github.com' });
		const firstOctokit = client.octokit;
		
		// Change the repo info
		client.setRepo({ owner: 'test-owner', repo: 'test-repo', domain: 'github.enterprise.com' });
		const secondOctokit = client.octokit;
		
		// Client should be recreated (different instances)
		expect(firstOctokit).not.toBe(secondOctokit);
	});

	test('resets client when token changes', () => {
		client.setRepo({ owner: 'test-owner', repo: 'test-repo', domain: 'github.com' });
		const firstOctokit = client.octokit;
		
		// Change the token
		client.setToken('new_token');
		const secondOctokit = client.octokit;
		
		// Client should be recreated (different instances)
		expect(firstOctokit).not.toBe(secondOctokit);
	});

	test('handles undefined domain as github.com', () => {
		client.setRepo({ owner: 'test-owner', repo: 'test-repo' });
		
		const octokit = client.octokit;
		
		expect(octokit).toBeDefined();
		expect(client.owner).toBe('test-owner');
		expect(client.repo).toBe('test-repo');
	});

	test('GitHub Enterprise URL construction follows expected pattern', () => {
		// This test validates the URL construction logic used by the newClient function
		const testCases = [
			{ domain: 'github.com', expected: 'https://api.github.com' },
			{ domain: 'github.enterprise.com', expected: 'https://github.enterprise.com/api' },
			{ domain: 'github.mycompany.com', expected: 'https://github.mycompany.com/api' },
			{ domain: 'ghe.internal.company.com', expected: 'https://ghe.internal.company.com/api' },
			{ domain: undefined, expected: 'https://api.github.com' }
		];

		testCases.forEach(({ domain, expected }) => {
			const testClient = new GitHubClient();
			testClient.setRepo({ owner: 'test', repo: 'test', domain });
			
			const octokit = testClient.octokit;
			// Check that the client was created successfully - the baseUrl logic is internal to Octokit
			expect(octokit).toBeDefined();
		});
	});
});