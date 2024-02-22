import { newClient } from '$lib/github/client';
import {
	type PullRequest,
	type GitHubIntegrationContext,
	ghResponseToInstance,
	type PrStatus,
	MergeMethod
} from '$lib/github/types';
import { showToast, type ToastMessage } from '$lib/notifications/toasts';
import { sleep } from '$lib/utils/sleep';
import * as toasts from '$lib/utils/toasts';
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
	distinct,
	map,
	retry,
	shareReplay,
	switchMap,
	tap,
	timeout
} from 'rxjs/operators';
import type { UserService } from '$lib/stores/user';
import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
import type { Octokit } from '@octokit/rest';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };

export class GitHubService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);

	private stateMap = new Map<string, BehaviorSubject<PrState>>();
	private reload$ = new BehaviorSubject<{ skipCache: boolean } | undefined>(undefined);
	private fresh$ = new Subject<void>();

	private ctx$: Observable<GitHubIntegrationContext | undefined>;
	private octokit$: Observable<Octokit | undefined>;

	// For use with user initiated actions like merging
	private ctx: GitHubIntegrationContext | undefined;
	private octokit: Octokit | undefined;

	private enabled = false;

	constructor(
		userService: UserService,
		private baseBranchService: BaseBranchService
	) {
		// A few things will cause the baseBranch to update, so we filter for distinct
		// changes to the remoteUrl.
		const distinctUrl$ = baseBranchService.base$.pipe(distinct((ctx) => ctx?.remoteUrl));

		this.ctx$ = combineLatest([userService.user$, distinctUrl$]).pipe(
			switchMap(([user, baseBranch]) => {
				const remoteUrl = baseBranch?.remoteUrl;
				const authToken = user?.github_access_token;
				const username = user?.github_username || '';
				if (!remoteUrl || !remoteUrl.includes('github') || !authToken) {
					this.enabled = false;
					return of();
				}
				this.enabled = true;
				const [owner, repo] = remoteUrl.split('.git')[0].split(/\/|:/).slice(-2);
				return of({ authToken, owner, repo, username });
			}),
			distinct((val) => JSON.stringify(val)),
			tap((ctx) => (this.ctx = ctx)),
			shareReplay(1)
		);

		// Create a github client
		this.octokit$ = this.ctx$.pipe(
			map((ctx) => (ctx ? newClient(ctx.authToken) : undefined)),
			tap((octokit) => (this.octokit = octokit)),
			shareReplay(1)
		);

		// Load pull requests
		this.prs$ = combineLatest([this.ctx$, this.octokit$, this.reload$]).pipe(
			tap(() => this.error$.next(undefined)),
			switchMap(([ctx, octokit, reload]) => {
				if (!ctx || !octokit) return EMPTY;
				const prs = loadPrs(ctx, octokit, !!reload?.skipCache);
				this.fresh$.next();
				return prs;
			}),
			shareReplay(1),
			catchError((err) => {
				console.log(err);
				this.error$.next(err);
				return of([]);
			})
		);
	}

	isEnabled(): boolean {
		return this.enabled;
	}

	get isEnabled$(): Observable<boolean> {
		return this.octokit$.pipe(map((octokit) => !!octokit));
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

	get(branch: string | undefined): Observable<PullRequest | undefined> | undefined {
		if (!branch) return;
		return this.prs$.pipe(map((prs) => prs.find((pr) => pr.targetBranch == branch)));
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
		if (!this.enabled) {
			throw "Can't create PR when service not enabled";
		}
		this.setBusy('creating_pr', branchId);
		return firstValueFrom(
			// We have to wrap with defer becasue using `async` functions with operators
			// create a promise that will stay rejected when rejected.
			defer(() =>
				combineLatest([this.octokit$, this.ctx$]).pipe(
					switchMap(async ([octokit, ctx]) => {
						if (!octokit || !ctx) {
							throw "Can't create PR without credentials";
						}
						try {
							const rsp = await octokit.rest.pulls.create({
								owner: ctx.owner,
								repo: ctx.repo,
								head: upstreamName,
								base,
								title,
								body,
								draft
							});
							await this.reload();
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
					})
				)
			).pipe(
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

                                \`\`\`${err.message}\`\`\`
                            `,
							style: 'error'
						});
						console.error('Unable to create PR despite retrying', err);
					}
					return throwError(() => err.message);
				}),
				tap(() => this.setIdle(branchId))
			)
		);
	}

	async getStatus(ref: string | undefined): Promise<PrStatus | undefined> {
		if (!ref || !this.octokit || !this.ctx) return;

		// Fetch with retries since checks might not be available _right_ after
		// the pull request has been created.
		const resp = await this.fetchChecksWithRetries(ref, 5, 2000);
		if (!resp) return;

		// If there are no checks then there is no status to report
		const checks = resp.data.check_runs;
		if (checks.length == 0) return;

		// Establish when the first check started running, useful for showing
		// how long something has been running.
		const starts = resp.data.check_runs
			.map((run) => run.started_at)
			.filter((startedAt) => startedAt !== null) as string[];
		const earliestStart = starts.map((startedAt) => new Date(startedAt));

		// TODO: This is wrong, we should not return here.
		if (earliestStart.length == 0) return;
		const firstStart = new Date(Math.min(...earliestStart.map((date) => date.getTime())));

		const skipped = checks.filter((c) => c.conclusion == 'skipped');
		const succeeded = checks.filter((c) => c.conclusion == 'success');
		// const failed = checks.filter((c) => c.conclusion == 'failure');
		const completed = checks.every((check) => !!check.completed_at);

		const count = resp?.data.total_count;
		return {
			startedAt: firstStart,
			success: skipped.length + succeeded.length == count,
			hasChecks: !!count,
			completed
		};
	}

	async fetchChecksWithRetries(ref: string, retries: number, delayMs: number) {
		let resp = await this.fetchChecks(ref);
		let retried = 0;

		while (retried < retries && (!resp || resp.data.total_count == 0)) {
			await sleep(delayMs);
			resp = await this.fetchChecks(ref);
			retried++;
		}
		return resp;
	}

	async fetchChecks(ref: string) {
		if (!ref || !this.octokit || !this.ctx) return;
		return await this.octokit.checks.listForRef({
			owner: this.ctx.owner,
			repo: this.ctx.repo,
			ref: ref,
			headers: {
				'X-GitHub-Api-Version': '2022-11-28'
			}
		});
	}

	async merge(pullNumber: number, method: MergeMethod) {
		if (!this.octokit || !this.ctx) return;
		try {
			await this.octokit.pulls.merge({
				owner: this.ctx.owner,
				repo: this.ctx.repo,
				pull_number: pullNumber,
				merge_method: method
			});
			await this.baseBranchService.fetchFromTarget();
		} finally {
			this.reload();
		}
	}
}

function loadPrs(
	ctx: GitHubIntegrationContext,
	octokit: Octokit,
	skipCache: boolean
): Observable<PullRequest[]> {
	return new Observable<PullRequest[]>((subscriber) => {
		const key = ctx.owner + '/' + ctx.repo;

		if (!skipCache) {
			const cachedRsp = lscache.get(key);
			if (cachedRsp) subscriber.next(cachedRsp.data.map(ghResponseToInstance));
		}

		try {
			octokit.rest.pulls
				.list({
					owner: ctx.owner,
					repo: ctx.repo
				})
				.then((rsp) => {
					lscache.set(key, rsp, 1440); // 1 day ttl
					subscriber.next(rsp.data.map(ghResponseToInstance));
				});
		} catch (e) {
			console.error(e);
		}
	});
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
 */
function mapErrorToToast(err: any): ToastMessage | undefined {
	// We expect an object to be thrown by octokit.
	if (typeof err != 'object') return;

	const { status, response } = err;
	const { data } = response;
	const { message, errors } = data;

	// If this expectation isn't met we must be doing something wrong
	if (status == undefined || message == undefined) return;

	if (message.includes('Draft pull requests are not supported')) {
		return {
			title: 'Draft pull requests are not enabled',
			message: `
                It looks like draft pull requests are not eanbled in your repository

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.

                \`\`\`${message}\`\`\`
            `,
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

                \`\`\`${message}\`\`\`
            `,
			style: 'error'
		};
	}

	if (message.includes('Validation Failed')) {
		let errorStrings = '';
		if (errors instanceof Array) {
			errorStrings = errors.map((err) => err.message).join('\n');
		}
		return {
			title: 'GitHub validation failed',
			message: `
                It looks like OAuth access has been restricted by your organization.

                Please see our [documentation](https://docs.gitbutler.com/)
                for additional help.

                \`\`\`${errorStrings}\`\`\`

            `,
			style: 'error'
		};
	}
}
