import { AzureDevOps } from '$lib/forge/azure/azure';
import { expect, test, describe } from 'vitest';

describe('AzureDevOps', () => {
	const baseRepo = {
		domain: 'dev.azure.com',
		name: 'test-repo',
		owner: 'test-owner',
		organization: 'test-org'
	};

	const baseBranch = 'main';

	test('commit url', () => {
		const azure = new AzureDevOps({
			repo: baseRepo,
			baseBranch,
			authenticated: false
		});

		expect(azure.commitUrl('abc123')).toBe(
			'https://dev.azure.com/test-org/test-owner/_git/test-repo/commit/abc123'
		);
	});

	test('uses https protocol for ssh remote urls (browser compatibility)', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh'
		};

		const azure = new AzureDevOps({
			repo,
			baseBranch,
			authenticated: false
		});

		expect(azure.commitUrl('abc123')).toBe(
			'https://dev.azure.com/test-org/test-owner/_git/test-repo/commit/abc123'
		);
	});

	test('branch urls use https protocol for ssh remote urls', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh'
		};

		const azure = new AzureDevOps({
			repo,
			baseBranch,
			authenticated: false
		});

		const branch = azure.branch('feature-branch');
		expect(branch?.url).toBe(
			'https://dev.azure.com/test-org/test-owner/_git/test-repo/branchCompare?baseVersion=GBmain&targetVersion=GBfeature-branch'
		);
	});

	test('handles ssh protocol with colon suffix', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh:'
		};

		const azure = new AzureDevOps({
			repo,
			baseBranch,
			authenticated: false
		});

		expect(azure.commitUrl('abc123')).toBe(
			'https://dev.azure.com/test-org/test-owner/_git/test-repo/commit/abc123'
		);
	});

	test('uses https protocol for ssh remote urls on custom Azure DevOps instance', () => {
		const repo = {
			domain: 'azuredevops.mycompany.com',
			name: 'test-repo',
			owner: 'test-owner',
			organization: 'test-org',
			protocol: 'ssh'
		};

		const azure = new AzureDevOps({
			repo,
			baseBranch,
			authenticated: false
		});

		expect(azure.commitUrl('abc123')).toBe(
			'https://azuredevops.mycompany.com/test-org/test-owner/_git/test-repo/commit/abc123'
		);
	});
});
