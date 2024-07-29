import { GitHub } from './github';
import { expect, test, describe } from 'vitest';

// TODO: Rewrite this proof-of-concept into something valuable.
describe.concurrent('GitHubBranch', () => {
	const name = 'some-branch';
	const base = 'some-base';
	const repo = {
		source: 'github.com',
		name: 'test-repo',
		owner: 'test-owner'
	};

	test('branch compare url', async () => {
		const gh = new GitHub(repo, base);
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${name}$`));
	});

	test('fork compare url', async () => {
		const fork = `${repo.owner}:${repo.name}`;
		const gh = new GitHub(repo, base, fork);
		const branch = gh.branch(name);
		expect(branch?.url).toMatch(new RegExp(`...${fork}:${name}$`));
	});
});
