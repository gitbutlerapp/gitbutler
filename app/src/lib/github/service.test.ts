import { GitHubService } from './service';
import { BehaviorSubject } from 'rxjs';
import { expect, test, describe } from 'vitest';

describe.concurrent('GitHubService', () => {
	describe.concurrent('parse GitHub remote URL', () => {
		test('default url', async () => {
			const accessToken$ = new BehaviorSubject<string | undefined>('token');
			const remoteUrl$ = new BehaviorSubject<string | undefined>(
				'git@github.com:gitbutlerapp/gitbutler'
			);

			const githubService = new GitHubService(accessToken$, remoteUrl$);

			expect(githubService.owner).toBe('gitbutlerapp');
			expect(githubService.repo).toBe('gitbutler');
		});
		test('ssh URL with .git suffix', async () => {
			const accessToken$ = new BehaviorSubject<string | undefined>('token');
			const remoteUrl$ = new BehaviorSubject<string | undefined>(
				'git@github.com:gitbutlerapp/gitbutler.git'
			);

			const githubService = new GitHubService(accessToken$, remoteUrl$);

			expect(githubService.owner).toBe('gitbutlerapp');
			expect(githubService.repo).toBe('gitbutler');
		});
		test('http URL', async () => {
			const accessToken$ = new BehaviorSubject<string | undefined>('token');
			const remoteUrl$ = new BehaviorSubject<string | undefined>(
				'https://github.com/gitbutlerapp/gitbutler'
			);

			const githubService = new GitHubService(accessToken$, remoteUrl$);

			expect(githubService.owner).toBe('gitbutlerapp');
			expect(githubService.repo).toBe('gitbutler');
		});
		test('http URL with .git suffix', async () => {
			const accessToken$ = new BehaviorSubject<string | undefined>('token');
			const remoteUrl$ = new BehaviorSubject<string | undefined>(
				'https://github.com/gitbutlerapp/gitbutler.git'
			);

			const githubService = new GitHubService(accessToken$, remoteUrl$);

			expect(githubService.owner).toBe('gitbutlerapp');
			expect(githubService.repo).toBe('gitbutler');
		});
	});
});
