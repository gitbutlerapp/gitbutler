import * as toasts from '$lib/utils/toasts';
import lscache from 'lscache';
import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
import type { UserService } from '$lib/stores/user';
import type { Octokit } from '@octokit/rest';
import { newClient } from '$lib/github/client';

import {
	type PullRequest,
	type GitHubIntegrationContext,
	ghResponseToInstance,
	type PrStatus
} from '$lib/github/types';
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

	private enabled = false;

	constructor(userService: UserService, baseBranchService: BaseBranchService) {
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
			shareReplay(1)
		);

		// Create a github client
		this.octokit$ = this.ctx$.pipe(
			map((ctx) => (ctx ? newClient(ctx.authToken) : undefined)),
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
		upstreamName: string
	): Promise<PullRequest | undefined> {
		if (!this.enabled) {
			console.error("Can't create PR when service not enabled");
			return;
		}
		this.setBusy('creating_pr', branchId);
		return firstValueFrom(
			// We have to wrap with defer becasue using `async` functions with operators
			// create a promise that will stay rejected when rejected.
			defer(() =>
				combineLatest([this.octokit$, this.ctx$]).pipe(
					switchMap(async ([octokit, ctx]) => {
						if (!octokit || !ctx) {
							console.error("Can't create PR without credentials");
							return;
						}
						const rsp = await octokit.rest.pulls.create({
							owner: ctx.owner,
							repo: ctx.repo,
							head: upstreamName,
							base,
							title,
							body
						});
						await this.reload();
						return ghResponseToInstance(rsp.data);
					})
				)
			).pipe(
				retry({
					count: 2,
					delay: 500
				}),
				timeout(60000), // 60 second total timeout
				catchError((err) => {
					if (err instanceof TimeoutError) {
						console.log('Timed out while trying to create pull request');
					} else {
						console.log('Unable to create PR despite retrying', err);
					}
					return throwError(() => err);
				}),
				tap(() => this.setIdle(branchId))
			)
		);
	}

	getStatus(ref: string | undefined): Observable<PrStatus | undefined> | undefined {
		if (!ref) return;
		return combineLatest([this.octokit$, this.ctx$]).pipe(
			switchMap(async ([octokit, ctx]) => {
				if (!octokit || !ctx) return;
				return await octokit.checks.listForRef({
					owner: ctx.owner,
					repo: ctx.repo,
					ref: ref,
					headers: {
						'X-GitHub-Api-Version': '2022-11-28'
					}
				});
			}),
			map((resp) => {
				if (!resp) return;
				const checks = resp?.data.check_runs;
				if (!checks) return;

				const skipped = checks.filter((c) => c.conclusion == 'skipped');
				const succeeded = checks.filter((c) => c.conclusion == 'success');
				const failed = checks.filter((c) => c.conclusion == 'failure');
				const completed = checks.every((check) => !!check.completed_at);

				const count = resp?.data.total_count;
				return {
					success: skipped.length + succeeded.length == count,
					hasChecks: !!count,
					completed,
					failed,
					skipped
				};
			})
		);
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
