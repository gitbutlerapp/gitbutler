import { GitHubService } from './service';
import { BehaviorSubject } from 'rxjs';
import { expect, test, describe } from 'vitest';

const exampleRemoteUrls = [
	'ssh://user@host.xz:123/org/repo.git/',
	'ssh://user@host.xz/org/repo.git/',
	'ssh://host.xz:123/org/repo.git/',
	'ssh://host.xz:123/org/repo',
	'ssh://host.xz/org/repo.git/',
	'ssh://host.xz/org/repo.git',
	'ssh://host.xz/org/repo',
	'ssh://user@host.xz/org/repo.git/',
	'ssh://user@host.xz/org/repo.git',
	'ssh://user@host.xz/org/repo',
	'host.xz:org/repo.git/',
	'host.xz:org/repo.git',
	'host.xz:org/repo',
	'user@host.xz:org/repo.git/',
	'user@host.xz:org/repo.git',
	'user@host.xz:org/repo',
	'git@github.com:org/repo.git/',
	'git@github.com:org/repo.git',
	'git@github.com:org/repo',
	'https://github.com/org/repo.git/',
	'https://github.com/org/repo.git',
	'https://github.com/org/repo'
];

describe.concurrent('GitHubService', () => {
	describe.concurrent('parse GitHub remote URL', () => {
		test.each(exampleRemoteUrls)('%s', (remoteUrl) => {
			const accessToken$ = new BehaviorSubject<string | undefined>('token');
			const remoteUrl$ = new BehaviorSubject<string | undefined>(remoteUrl);

			const githubService = new GitHubService(accessToken$, remoteUrl$);

			expect(githubService.owner).toBe('org');
			expect(githubService.repo).toBe('repo');
		});
	});
});
