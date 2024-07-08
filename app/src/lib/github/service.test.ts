import { GitHubService } from './service';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { BehaviorSubject } from 'rxjs';
import { expect, test, describe } from 'vitest';

const exampleRemoteUrls = [
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

			const projectMetrics = new ProjectMetrics();
			const githubService = new GitHubService(projectMetrics, accessToken$, remoteUrl$);

			expect(githubService.owner).toBe('org');
			expect(githubService.repo).toBe('repo');
		});
	});
});
