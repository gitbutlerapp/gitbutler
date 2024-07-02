import {
	type PullRequest,
	ghResponseToInstance,
	type ChecksStatus,
	MergeMethod,
	type DetailedPullRequest,
	parseGitHubDetailedPullRequest as parsePullRequestResponse,
	parseGitHubCheckSuites,
	type CheckSuites
} from '$lib/github/types';
import { showError, showToast, type Toast } from '$lib/notifications/toasts';
import { sleep } from '$lib/utils/sleep';
import * as toasts from '$lib/utils/toasts';
import { Octokit } from '@octokit/rest';
import GitUrlParse from 'git-url-parse';
import lscache from 'lscache';
import posthog from 'posthog-js';
import {
	Observable,
	EMPTY,
	BehaviorSubject,
	of,
	firstValueFrom,
	Subject,
	combineLatest,
	defer,
	TimeoutError,
	throwError
} from 'rxjs';
import {
	catchError,
	delay,
	finalize,
	map,
	retry,
	shareReplay,
	switchMap,
	take,
	tap,
	timeout
} from 'rxjs/operators';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

const DEFAULT_HEADERS = {
	'X-GitHub-Api-Version': '2022-11-28',
	'If-None-Match': ''
};

export class GitHubService {
	readonly prs$ = new BehaviorSubject<PullRequest[]>([]);

	private prCache = new Map<string, PrCacheKey>();

	private error$ = new BehaviorSubject<string | undefined>(undefined);
	private stateMap = new Map<string, BehaviorSubject<PrState>>();
	private reload$ = new BehaviorSubject<{ skipCache: boolean } | undefined>(undefined);
	private fresh$ = new Subject<void>();

	private _octokit: Octokit | undefined;
	private _repo: string | undefined;
	private _owner: string | undefined;

	constructor(
		accessToken$: Observable<string | undefined>,
		remoteUrl$: Observable<string | undefined>
	) {
		combineLatest([accessToken$, remoteUrl$])
			.pipe(
				tap(([accessToken, remoteUrl]) => {
					// We check the remote url since GitHub is currently enabled at the account
					// level rather than project level.
					if (!accessToken || !remoteUrl?.includes('github.com')) {
						return of();
					}
					this._octokit = new Octokit({
						auth: accessToken,
						userAgent: 'GitButler Client',
						baseUrl: 'https://api.github.com'
					});
					if (remoteUrl) {
						const { owner, name } = GitUrlParse(remoteUrl);
						this._repo = name;
						this._owner = owner;
					}
				}),
				shareReplay(1)
			)
			.subscribe();

		combineLatest([this.reload$, accessToken$, remoteUrl$])
			.pipe(
				tap(() => this.error$.next(undefined)),
				switchMap(([reload, _token, _remoteUrl]) => {
					if (!this._octokit || !this._owner) return EMPTY;
					const prs = this.fetchPrs(!!reload?.skipCache);
					this.fresh$.next();
					return prs;
				}),
				shareReplay(1),
				catchError((err) => {
					console.error(err);
					showError('Failed to load pull requests', err);
					this.error$.next(err);
					return of([]);
				}),
				tap((prs) => this.prs$.next(prs))
			)
			.subscribe();
	}

	get isEnabled(): boolean {
		return !!this._octokit;
	}

	get octokit(): Octokit {
		if (!this._octokit) throw new Error('No GitHub client available');
		return this._octokit;
	}

	get repo(): string {
		if (!this._repo) throw new Error('No repo name specified');
		return this._repo;
	}

	get owner(): string {
		if (!this._owner) throw new Error('No owner name specified');
		return this._owner;
	}

	get prs() {
		return this.prs$.value;
	}

	async reload(): Promise<void> {
		const fresh = firstValueFrom(
			this.fresh$.pipe(
				timeout(30000),
				catchError(() => {
					// Observable never errors for any other reasons
					console.warn('Timed out while reloading pull requests');
					toasts.error('Timed out while reloading pull requests');
					return of();
				})
			)
		);
		this.reload$.next({ skipCache: true });
		return await fresh;
	}

	fetchPrs(skipCache: boolean): Observable<PullRequest[]> {
		return new Observable<PullRequest[]>((subscriber) => {
			const key = this.owner + '/' + this.repo;

			if (!skipCache) {
				const cachedRsp = lscache.get(key);
				if (cachedRsp?.data) subscriber.next(cachedRsp.data.map(ghResponseToInstance));
			}

			try {
				this.octokit.rest.pulls
					.list({
						headers: DEFAULT_HEADERS,
						owner: this.owner,
						repo: this.repo
					})
					.then((rsp) => {
						lscache.set(key, rsp, 1440); // 1 day ttl
						subscriber.next(rsp.data.map(ghResponseToInstance));
					})
					.catch((e) => subscriber.error(e));
			} catch (e) {
				console.error(e);
			}
		});
	}

	async getDetailedPr(
		branchSha: string,
		skipCache: boolean
	): Promise<DetailedPullRequest | undefined> {
		const cachedPr = !skipCache && this.prCache.get(branchSha);
		if (cachedPr) {
			const cacheTimeMs = 2 * 1000;
			const age = new Date().getTime() - cachedPr.fetchedAt.getTime();
			if (age < cacheTimeMs) return cachedPr.value;
		}

		const prNumber = this.getListedPr(branchSha)?.number;
		if (!prNumber) {
			toasts.error('No pull request number for branch ' + branchSha);
			return;
		}

		const request = async () => {
			const resp = await this.octokit.pulls.get({
				headers: DEFAULT_HEADERS,
				owner: this.owner,
				repo: this.repo,
				pull_number: prNumber
			});
			return parsePullRequestResponse(resp.data);
		};

		// Right after pushing GitHub will respond with status 422, retries are necessary
		let attempt = 0;
		let pr = await request();

		while (!pr && attempt < 3) {
			attempt++;
			try {
				pr = await request();
				if (pr) this.prCache.set(branchSha, { value: pr, fetchedAt: new Date() });
				return pr;
			} catch (err: any) {
				if (err.status !== 422) throw err;
				await sleep(1000);
			}
		}
		if (!pr) throw 'Failed to fetch pull request details';
		return pr;
	}

	async shouldWaitForChecks(ref: string) {
		const resp = await this.getCheckSuites(ref);
		const checkSuites = resp?.items;
		if (!checkSuites) return true;

		// Continue waiting if some check suites are in progress
		if (checkSuites.some((suite) => suite.status !== 'completed')) return true;
	}

	getListedPr(branchSha: string): PullRequest | undefined {
		return this.prs?.find((pr) => pr.sha === branchSha);
	}

	getPr$(branchSha: string): Observable<PullRequest | undefined> {
		return this.prs$.pipe(map((prs) => prs.find((pr) => pr.sha === branchSha)));
	}

	/* TODO: Figure out a way to cleanup old behavior subjects */
	getState(branchId: string) {
		let state$ = this.stateMap.get(branchId);
		if (!state$) {
			state$ = new BehaviorSubject<PrState>({ busy: false, branchId });
			this.stateMap.set(branchId, state$);
		}
		return state$;
	}

	private setBusy(action: PrAction, branchId: string) {
		const state$ = this.getState(branchId);
		state$.next({ busy: true, action, branchId });
	}

	private setIdle(branchId: string) {
		const state$ = this.getState(branchId);
		state$.next({ busy: false, branchId });
	}

	async createPullRequest(
		base: string,
		title: string,
		body: string,
		branchId: string,
		upstreamName: string,
		draft: boolean
	): Promise<{ pr: PullRequest } | { err: string | { message: string; help: string } }> {
		this.setBusy('creating_pr', branchId);
		return await firstValueFrom(
			// We have to wrap with defer becasue using `async` functions with operators
			// create a promise that will stay rejected when rejected.
			defer(async () => {
				try {
					const rsp = await this.octokit.rest.pulls.create({
						owner: this.owner,
						repo: this.repo,
						head: upstreamName,
						base,
						title,
						body,
						draft
					});
					posthog.capture('PR Successful');
					return { pr: ghResponseToInstance(rsp.data) };
				} catch (err: any) {
					const toast = mapErrorToToast(err);
					if (toast) {
						// TODO: This needs disambiguation, not the same as `toasts.error`
						// Show toast with rich content
						showToast(toast);
						// Handled errors should not be retried
						return { err };
					} else {
						// Rethrow so that error is retried
						throw err;
					}
				}
			}).pipe(
				retry({
					count: 2,
					delay: 500
				}),
				timeout(60000), // 60 second total timeout
				catchError((err) => {
					this.setIdle(branchId);

					// TODO: Perhaps we should only capture part of the error object
					posthog.capture('PR Failed', { error: err });

					if (err instanceof TimeoutError) {
						showToast({
							title: 'Timed out while creating PR',
							message: `
                                We are not certain whether it was created successfully or not,
                                please sync to verify.

                                You can also see our [documentation](https://docs.gitbutler.com/)
                                for additional help.
                            `,
							style: 'error'
						});
						console.error('Timed out while trying to create pull request', err);
					} else {
						showToast({
							title: 'Failed to create PR despite retrying',
							message: `
                                Please check your GitHub authentication settings and try again.

                                You can also see our [documentation](https://docs.gitbutler.com/)
                                for additional help.
                            `,
							error: err.message,
							style: 'error'
						});
						console.error('Unable to create PR despite retrying', err);
					}
					return throwError(() => err.message);
				}),
				tap(() => this.setIdle(branchId)),
				// Makes finalize happen after first and only result
				take(1),
				// Wait for GitHub to become eventually consistent. If we refresh too quickly then
				// then it'll show as mergeable and no checks even if checks are present.
				delay(1000),
				finalize(async () => await this.reload())
			)
		);
	}

	async checks(ref: string | undefined): Promise<ChecksStatus | null> {
		if (!ref) return null;

		// Fetch with retries since checks might not be available _right_ after
		// the pull request has been created.
		const resp = await this.fetchChecksWithRetries(ref, 5, 2000);

		// If there are no checks then there is no status to report
		const checks = resp.data.check_runs;
		if (checks.length === 0) return null;

		// Establish when the first check started running, useful for showing
		// how long something has been running.
		const starts = resp.data.check_runs
			.map((run) => run.started_at)
			.filter((startedAt) => startedAt !== null) as string[];
		const startTimes = starts.map((startedAt) => new Date(startedAt));

		const queued = checks.filter((c) => c.status === 'queued').length;
		const failed = checks.filter((c) => c.conclusion === 'failure').length;
		const skipped = checks.filter((c) => c.conclusion === 'skipped').length;
		const succeeded = checks.filter((c) => c.conclusion === 'success').length;

		const firstStart = new Date(Math.min(...startTimes.map((date) => date.getTime())));
		const completed = checks.every((check) => !!check.completed_at);
		const totalCount = resp?.data.total_count;

		const success = queued === 0 && failed === 0 && skipped + succeeded === totalCount;
		const finished = checks.filter(
			(c) => c.conclusion && ['failure', 'success'].includes(c.conclusion)
		).length;

		return {
			startedAt: firstStart,
			hasChecks: !!totalCount,
			success,
			failed,
			completed,
			queued,
			totalCount,
			skipped,
			finished
		};
	}

	async getCheckSuites(ref: string | undefined): Promise<CheckSuites> {
		if (!ref) return null;
		const resp = await this.octokit.checks.listSuitesForRef({
			headers: DEFAULT_HEADERS,
			owner: this.owner,
			repo: this.repo,
			ref: ref
		});
		return { count: resp.data.total_count, items: parseGitHubCheckSuites(resp.data) };
	}

	async fetchChecks(ref: string) {
		return await this.octokit.checks.listForRef({
			headers: DEFAULT_HEADERS,
			owner: this.owner,
			repo: this.repo,
			ref: ref
		});
	}

	async fetchChecksWithRetries(ref: string, retries: number, delayMs: number) {
		let resp = await this.fetchChecks(ref);
		let retried = 0;
		let shouldWait: boolean | undefined = undefined;

		while (resp.data.total_count === 0 && retried < retries) {
			if (shouldWait === undefined && retried > 0) {
				shouldWait = await this.shouldWaitForChecks(ref);
				if (!shouldWait) {
					return resp;
				}
			}
			await sleep(delayMs);
			resp = await this.fetchChecks(ref);
			retried++;
		}
		return resp;
	}

	async merge(pullNumber: number, method: MergeMethod) {
		try {
			return await this.octokit.pulls.merge({
				owner: this.owner,
				repo: this.repo,
				pull_number: pullNumber,
				merge_method: method
			});
		} finally {
			this.reload();
		}
	}

	async fetchGitHubLogin(): Promise<string> {
		try {
			const rsp = await this.octokit.users.getAuthenticated();
			return rsp.data.login;
		} catch (e) {
			console.error(e);
			throw e;
		}
	}
}

/**
 * Example error responses
 * ```
 * {
 *   "name": "HttpError",
 *   "request": {
 *      "body": "{\"head\":\"branch1\",\"base\":\"main\",\"title\":\"Some title\",\"body\":\"\",\"draft\":false}",
 *      "headers": {
 *        "accept": "application/vnd.github.v3+json",
 *        "authorization": "token [REDACTED]",
 *        "content-type": "application/json; charset=utf-8",
 *        "user-agent": "GitButler Client octokit-rest.js/20.0.2 octokit-core.js/5.0.1 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)"
 *      },
 *      "method": "POST",
 *      "request": {
 *        "hook": {}
 *      "url": "https://api.github.com/repos/someuser/somerepo/pulls"
 *     },
 *     "response": {
 *       "data": {
 *         "documentation_url": "https://docs.github.com/rest/pulls/pulls#create-a-pull-request",
 *         "message": "Although you appear to have the correct authorization credentials, the organization has enabled OAuth App access restrictions, meaning that data access to third-parties is limited. For more information on these restrictions, including how to enable this app, visit https://docs.github.com/articles/restricting-access-to-your-organization-s-data/"
 *       },
 *       "headers": {
 *         "content-type": "application/json; charset=utf-8",
 *         "x-accepted-oauth-scopes": "",
 *         "x-github-media-type": "github.v3; format=json",
 *         "x-github-request-id": "F93F:2A6A69:1CEAF8:1D2E00:65D486FC",
 *         "x-oauth-scopes": "repo",
 *         "x-ratelimit-limit": "5000",
 *         "x-ratelimit-remaining": "4968",
 *         "x-ratelimit-reset": "1708427744",
 *         "x-ratelimit-resource": "core",
 *         "x-ratelimit-used": "32"
 *       },
 *       "status": 403,
 *       "url": "https://api.github.com/repos/someuser/somerepo/pulls"
 *     },
 *   "status": 403
 * }
 *
 * {
 *   name: 'HttpError',
 *   request: {
 *     body: '{"head":"branch2","base":"main","title":"some title","body":"","draft":false}',
 *     headers: {
 *       accept: 'application/vnd.github.v3+json',
 *       authorization: 'token [REDACTED]',
 *       'content-type': 'application/json; charset=utf-8',
 *       'user-agent': 'GitButler Client octokit-rest.js/20.0.2 octokit-core.js/5.0.1 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)'
 *     },
 *     method: 'POST',
 *     request: {
 *         hook: {}
 *     },
 *     url: 'https://api.github.com/repos/someuser/somerepo/api/pulls'
 *   },
 *   response: {
 *     data: {
 *       documentation_url:
 *           'https://docs.github.com/rest/pulls/pulls#create-a-pull-request',
 *       errors: [
 *         {
 *            code: 'custom',
 *            message: 'A pull request already exists for someuser:somebranch.',
 *            resource: 'PullRequest'
 *         }
 *       ],
 *       message: 'Validation Failed'
 *     },
 *     headers: {
 *       'content-length': '266',
 *       'content-type': 'application/json; charset=utf-8',
 *       'x-accepted-oauth-scopes': '',
 *       'x-github-media-type': 'github.v3; format=json',
 *       'x-github-request-id': 'C233:72D21:6493:6C61:65D366B1',
 *       'x-oauth-scopes': 'repo',
 *       'x-ratelimit-limit': '5000',
 *       'x-ratelimit-remaining': '4994',
 *       'x-ratelimit-reset': '1708356743',
 *       'x-ratelimit-resource': 'core',
 *       'x-ratelimit-used': '6'
 *     },
 *     status: 422,
 *     url: 'https://api.github.com/repos/someuser/somerepo/pulls'
 *   },
 *   status: 422
 * }
 * ```
 *
 * {
 *   "name": "HttpError",
 *   "request": {
 *     "body": "{\"head\":\"Update-vscode-colors\",\"base\":\"C1-393-docker-implementation\",\"title\":\"Update vscode colors\",\"body\":\"\",\"draft\":false}",
 *     "headers": {
 *       "accept": "application/vnd.github.v3+json",
 *       "authorization": "token [REDACTED]",
 *       "content-type": "application/json; charset=utf-8",
 *       "user-agent": "GitButler Client octokit-rest.js/20.0.2 octokit-core.js/5.0.1 Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)"
 *     },
 *     "method": "POST",
 *     "request": {
 *       "hook": {}
 *     },
 *     "url": "https://api.github.com/repos/c1-ab/c1-backend/pulls"
 *   },
 *   "response": {
 *     "data": {
 *       "documentation_url": "https://docs.github.com/rest/pulls/pulls#create-a-pull-request",
 *       "errors": [
 *         {
 *           "code": "invalid",
 *           "field": "base",
 *           "resource": "PullRequest"
 *         }
 *       ],
 *       "message": "Validation Failed"
 *     },
 *     "headers": {
 *       "content-length": "186",
 *       "content-type": "application/json; charset=utf-8",
 *       "x-accepted-oauth-scopes": "",
 *       "x-github-media-type": "github.v3; format=json",
 *       "x-github-request-id": "E5EE:F1F0:6880D:6984F:65D74AC3",
 *       "x-oauth-scopes": "repo",
 *       "x-ratelimit-limit": "15000",
 *       "x-ratelimit-remaining": "14950",
 *       "x-ratelimit-reset": "1708609120",
 *       "x-ratelimit-resource": "core",
 *       "x-ratelimit-used": "50"
 *     },
 *     "status": 422,
 *     "url": "https://api.github.com/repos/c1-ab/c1-backend/pulls"
 *   },
 *   "status": 422
 * }
 */
function mapErrorToToast(err: any): Toast | undefined {
	// We expect an object to be thrown by octokit.
	if (typeof err !== 'object') return;

	const { status, response } = err;
	const { data } = response;
	const { message, errors } = data;

	// If this expectation isn't met we must be doing something wrong
	if (status === undefined || message === undefined) return;

	if (message.includes('Draft pull requests are not supported')) {
		return {
			title: 'Draft pull requests are not enabled',
			message: `
                It looks like draft pull requests are not enabled in your repository.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: message,
			style: 'error'
		};
	}

	if (message.includes('enabled OAuth App access restrictions')) {
		return {
			title: 'OAuth access restricted',
			message: `
                It looks like OAuth access has been restricted by your organization.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: message,
			style: 'error'
		};
	}

	if (message.includes('Validation Failed')) {
		let errorStrings = '';
		if (errors instanceof Array) {
			errorStrings = errors
				.map((err) => {
					if (err.message) return err.message;
					if (err.field && err.code) return `${err.field} ${err.code}`;
					return 'unknown validation error';
				})
				.join('\n');
		}
		return {
			title: 'GitHub validation failed',
			message: `
                It seems there was a problem validating the request.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.
            `,
			error: errorStrings,
			style: 'error'
		};
	}
}
