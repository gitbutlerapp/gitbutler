import { BaseBranch } from './types';
import { Code, invoke } from '$lib/backend/ipc';
import { observableToStore } from '$lib/rxjs/store';
import * as toasts from '$lib/utils/toasts';
import { plainToInstance } from 'class-transformer';
import {
	switchMap,
	Observable,
	shareReplay,
	catchError,
	BehaviorSubject,
	debounceTime,
	tap,
	combineLatest,
	startWith,
	Subject,
	mergeWith
} from 'rxjs';
import type { Readable } from 'svelte/store';

export class NoDefaultTarget extends Error {}

export class BaseBranchService {
	readonly base$: Observable<BaseBranch | null | undefined>;
	readonly busy$ = new BehaviorSubject(false);
	readonly error$ = new BehaviorSubject<any>(undefined);
	private readonly reload$ = new Subject<void>();

	readonly base: Readable<BaseBranch | null | undefined>;
	readonly error: Readable<any>;

	constructor(
		private readonly projectId: string,
		readonly remoteUrl$: BehaviorSubject<string | undefined>,
		fetches$: Observable<unknown>,
		readonly head$: Observable<string>
	) {
		this.base$ = combineLatest([fetches$, head$]).pipe(
			mergeWith(this.reload$),
			debounceTime(100),
			switchMap(async () => {
				this.busy$.next(true);
				const baseBranch = await getBaseBranch({ projectId });
				if (!baseBranch) {
					throw new NoDefaultTarget();
				}
				this.busy$.next(false);
				return baseBranch;
			}),
			tap((baseBranch) => {
				if (baseBranch?.remoteUrl) this.remoteUrl$.next(baseBranch.remoteUrl);
			}),
			catchError((e) => {
				this.remoteUrl$.next(undefined);
				this.busy$.next(false);
				throw e;
			}),
			// Start with undefined to prevent delay in updating $baseBranch value in
			// layout.svelte when navigating between projects.
			startWith(undefined),
			shareReplay(1)
		);
		[this.base, this.error] = observableToStore(this.base$, this.reload$);
	}

	async fetchFromTarget(action: string | undefined = undefined) {
		this.busy$.next(true);
		try {
			// Note that we expect the back end to emit new fetches event, and therefore
			// trigger a base branch reload. It feels a bit awkward and should be improved.
			await invoke<void>('fetch_from_target', {
				projectId: this.projectId,
				action: action || 'auto'
			});
		} catch (err: any) {
			if (err.message?.includes('does not have a default target')) {
				// Swallow this error since user should be taken to project setup page
				return;
			} else if (err.code === Code.ProjectsGitAuth) {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`${err.message}`);
			}
			console.error(err);
		} finally {
			this.busy$.next(false);
		}
	}

	async setTarget(branch: string) {
		this.busy$.next(true);
		await invoke<BaseBranch>('set_base_branch', { projectId: this.projectId, branch });
		await this.fetchFromTarget();
	}

	reload() {
		this.busy$.next(true);
		this.reload$.next();
	}
}

async function getBaseBranch(params: { projectId: string }): Promise<BaseBranch | null> {
	return plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params));
}

export async function getRemoteBranches(projectId: string | undefined) {
	if (!projectId) return [];
	return await invoke<Array<string>>('git_remote_branches', { projectId }).then((branches) =>
		branches
			.map((name) => name.substring(13))
			.sort((a, b) => a.localeCompare(b))
			.map((name) => ({ name }))
	);
}
