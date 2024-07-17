import { createBaseBranch } from './baseBranch';
import { BaseBranch, type ForgeType } from './types';
import { expect, test, describe } from 'vitest';

const forgeTestUrls: Record<ForgeType, Array<string>> = {
	GitHub: [
		'git@github.com:org/repo.git/',
		'git@github.com:org/repo.git',
		'git@github.com:org/repo',
		'https://github.com/org/repo.git/',
		'https://github.com/org/repo.git',
		'https://github.com/org/repo',
		'ssh://git@github.com:22/org/repo.git'
	],
	GitLab: ['git@gitlab.com:org/repo.git', 'https://gitlab.com/org/repo.git'],
	Bitbucket: ['git@bitbucket.org:org/repo.git', 'https://user@bitbucket.org/org/repo.git'],
	AzureDevOps: [
		'https://user@dev.azure.com/org/project/_git/repo',
		'git@ssh.dev.azure.com:v3/org/project/repo'
	],
	Unknown: ['https://otherdomain.com/org/repo']
};

describe.concurrent('BaseBranch', () => {
	describe.each(Object.keys(forgeTestUrls).map((_) => _ as ForgeType))(
		'parse %s as forge',
		(forgeKey: ForgeType) => {
			test('test collection not empty', () => {
				expect(forgeTestUrls[forgeKey]).toBeTruthy();
				expect(forgeTestUrls[forgeKey].length).not.toBe(0);
			});

			test.each(forgeTestUrls[forgeKey])('%s', (remoteUrl: string) => {
				const baseBranch: BaseBranch = createBaseBranch({
					remoteUrl: remoteUrl
				});

				expect(baseBranch.forgeType).toBe(forgeKey);
			});
		}
	);

	// The BranchService and get_base_branch_data inside of Rust should validate that this is always supplied but the
	// previous typescript/svelte implementation supported this not being supplied so this one does as well
	test('no pushRemoteUrl, does not throw', () => {
		const baseBranch: BaseBranch = createBaseBranch({
			remoteUrl: 'https://github.com/org/repo.git',
			pushRemoteUrl: undefined
		});
		expect(baseBranch.commitBaseUrl).toEqual(undefined);
	});

	test('it should handle complete ssh url with ip', () => {
		const remoteUrl = 'ssh://git@192.168.1.1:22/org/repo.git';
		const baseBranch: BaseBranch = createBaseBranch({
			remoteUrl: remoteUrl,
			pushRemoteUrl: remoteUrl
		});
		expect(baseBranch.commitBaseUrl).toEqual('http://192.168.1.1/org/repo');
	});

	describe.each(Object.keys(forgeTestUrls))(
		'parse %s for commit base url',
		(forgeKey: ForgeType) => {
			test('test collection not empty', () => {
				expect(forgeTestUrls[forgeKey]).toBeTruthy();
				expect(forgeTestUrls[forgeKey].length).not.toBe(0);
			});

			test.each(forgeTestUrls[forgeKey])('%s', (remoteUrl: string) => {
				const baseBranch: BaseBranch = createBaseBranch({
					remoteUrl: remoteUrl,
					pushRemoteUrl: remoteUrl
				});

				switch (forgeKey) {
					case 'GitHub':
						expect(baseBranch.commitBaseUrl).toBe('https://github.com/org/repo');
						break;
					case 'GitLab':
						expect(baseBranch.commitBaseUrl).toBe('https://gitlab.com/org/repo');
						break;
					case 'Bitbucket':
						expect(baseBranch.commitBaseUrl).toBe('https://bitbucket.org/org/repo');
						break;
					case 'AzureDevOps':
						expect(baseBranch.commitBaseUrl).toBe('https://dev.azure.com/org/project/_git/repo');
						break;
					case 'Unknown':
						expect(baseBranch.commitBaseUrl).toBe('https://otherdomain.com/org/repo');
						break;
				}
			});
		}
	);

	describe.each(Object.keys(forgeTestUrls))('parse %s for commit url', (forgeKey: ForgeType) => {
		test('test collection not empty', () => {
			expect(forgeTestUrls[forgeKey]).toBeTruthy();
			expect(forgeTestUrls[forgeKey].length).not.toBe(0);
		});

		test.each(forgeTestUrls[forgeKey])('%s', (remoteUrl: string) => {
			const baseBranch: BaseBranch = createBaseBranch({
				remoteUrl: remoteUrl,
				pushRemoteUrl: remoteUrl
			});

			const commitId = 'abcdef';
			switch (forgeKey) {
				case 'GitHub':
					expect(baseBranch.commitUrl(commitId)).toBe(
						`https://github.com/org/repo/commit/${commitId}`
					);
					break;
				case 'GitLab':
					expect(baseBranch.commitUrl(commitId)).toBe(
						`https://gitlab.com/org/repo/-/commit/${commitId}`
					);
					break;
				case 'Bitbucket':
					expect(baseBranch.commitUrl(commitId)).toBe(
						`https://bitbucket.org/org/repo/commits/${commitId}`
					);
					break;
				case 'AzureDevOps':
					expect(baseBranch.commitUrl(commitId)).toBe(
						`https://dev.azure.com/org/project/_git/repo/commit/${commitId}`
					);
					break;
				case 'Unknown':
					expect(baseBranch.commitUrl(commitId)).toBe(
						`https://otherdomain.com/org/repo/commit/${commitId}`
					);
					break;
			}
		});
	});

	test('null is handled', () => {
		const baseBranch: BaseBranch = createBaseBranch(null);

		// Validate how it's handled in code
		expect(baseBranch).toBeFalsy();
		// Validate it's actually null
		expect(baseBranch).toBeNull();
	});
});
