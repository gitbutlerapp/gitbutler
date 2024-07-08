import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { observableToStore } from '$lib/rxjs/store';
import { RemoteBranch, RemoteBranchData } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';
import {
	Observable,
	Subject,
	catchError,
	combineLatest,
	mergeWith,
	shareReplay,
	switchMap,
	tap
} from 'rxjs';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { Readable } from 'svelte/store';

export class RemoteBranchService {
	branches: Readable<RemoteBranch[] | undefined>;
	branches$: Observable<RemoteBranch[]>;
	error: Readable<string | undefined>;
	private reload$ = new Subject<void>();

	constructor(
		projectId: string,
		private projectMetrics: ProjectMetrics,
		fetches$: Observable<any>,
		head$: Observable<any>,
		baseBranch$: Observable<any>
	) {
		this.branches$ = combineLatest([baseBranch$, head$, fetches$]).pipe(
			mergeWith(this.reload$),
			switchMap(async () => await listRemoteBranches(projectId)),
			tap((branches) => {
				this.projectMetrics.setMetric('normal_branch_count', branches.length);
			}),
			shareReplay(1),
			catchError((e) => {
				console.error(e);
				showError('Failed load remote branches', e);
				throw e;
			})
		);
		[this.branches, this.error] = observableToStore(this.branches$, this.reload$);
	}

	reload() {
		this.reload$.next();
	}
}

async function listRemoteBranches(projectId: string): Promise<RemoteBranch[]> {
	const branches = plainToInstance(
		RemoteBranch,
		await invoke<any[]>('list_remote_branches', { projectId })
	);

	return branches;
}

export async function getRemoteBranchData(
	projectId: string,
	refname: string
): Promise<RemoteBranchData> {
	return plainToInstance(
		RemoteBranchData,
		await invoke<any>('get_remote_branch_data', { projectId, refname })
	);
}
