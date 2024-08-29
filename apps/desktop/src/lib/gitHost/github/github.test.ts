import { GitHub } from './github';
import { expect, test, describe } from 'vitest';

describe.concurrent('GitHub', () => {
	const id = 'some-branch';
	const repo = {
		source: 'github.com',
		name: 'test-repo',
		owner: 'test-owner',
		resource: 'test-resource'
	};

	test('commit url', async () => {
		const gh = new GitHub({ repo, baseBranch: id });
		const url = gh.commitUrl(id);
		expect(url).toMatch(new RegExp(`/${id}$`));
	});
});
