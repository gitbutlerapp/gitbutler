import { BitBucket } from '$lib/forge/bitbucket/bitbucket';
import { expect, test, describe } from 'vitest';

describe('BitBucket', () => {
	const baseRepo = {
		domain: 'bitbucket.org',
		name: 'test-repo',
		owner: 'test-owner'
	};

	const baseBranch = 'main';

	test('commit url', () => {
		const bb = new BitBucket({
			repo: baseRepo,
			baseBranch,
			authenticated: false
		});

		expect(bb.commitUrl('abc123')).toBe(
			'https://bitbucket.org/test-owner/test-repo/commits/abc123'
		);
	});

	test('uses https protocol for ssh remote urls (browser compatibility)', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh'
		};

		const bb = new BitBucket({
			repo,
			baseBranch,
			authenticated: false
		});

		expect(bb.commitUrl('abc123')).toBe(
			'https://bitbucket.org/test-owner/test-repo/commits/abc123'
		);
	});

	test('branch urls use https protocol for ssh remote urls', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh'
		};

		const bb = new BitBucket({
			repo,
			baseBranch,
			authenticated: false
		});

		const branch = bb.branch('feature-branch');
		expect(branch?.url).toBe(
			'https://bitbucket.org/test-owner/test-repo/branch/feature-branch?dest=main'
		);
	});

	test('handles ssh protocol with colon suffix', () => {
		const repo = {
			...baseRepo,
			protocol: 'ssh:'
		};

		const bb = new BitBucket({
			repo,
			baseBranch,
			authenticated: false
		});

		expect(bb.commitUrl('abc123')).toBe(
			'https://bitbucket.org/test-owner/test-repo/commits/abc123'
		);
	});

	test('uses https protocol for ssh remote urls on custom BitBucket instance', () => {
		const repo = {
			domain: 'bitbucket.mycompany.com',
			name: 'test-repo',
			owner: 'test-owner',
			protocol: 'ssh'
		};

		const bb = new BitBucket({
			repo,
			baseBranch,
			authenticated: false
		});

		expect(bb.commitUrl('abc123')).toBe(
			'https://bitbucket.mycompany.com/test-owner/test-repo/commits/abc123'
		);
	});
});
