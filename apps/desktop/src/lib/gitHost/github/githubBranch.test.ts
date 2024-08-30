import { GitHub } from './github';
import { expect, test, describe } from 'vitest';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubBranch', () => {
	const name = 'some-branch';
	const baseBranch = 'some-base';
	const repo = {
		domain: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	test('branch compare url', async () => {
		const gh = new GitHub({ repo, baseBranch });
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${name}$`));
	});

	test('fork compare url', async () => {
		const forkStr = `${repo.owner}:${repo.name}`;
		const gh = new GitHub({ repo, baseBranch, forkStr });
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${forkStr}:${name}$`));
	});
});
