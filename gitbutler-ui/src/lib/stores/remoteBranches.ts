import { invoke } from '$lib/backend/ipc';
import { observableToStore } from '$lib/rxjs/store';
import * as toasts from '$lib/utils/toasts';
import { RemoteBranch, RemoteBranchData } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';
import {
	BehaviorSubject,
	Observable,
	catchError,
	combineLatest,
	shareReplay,
	switchMap
} from 'rxjs';
import type { Readable } from 'svelte/store';

export class RemoteBranchService {
	branches: Readable<RemoteBranch[] | undefined>;
	branches$: Observable<RemoteBranch[]>;
	error: Readable<string | undefined>;
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(
		projectId: string,
		fetches$: Observable<any>,
		head$: Observable<any>,
		baseBranch$: Observable<any>
	) {
		this.branches$ = combineLatest([baseBranch$, this.reload$, head$, fetches$]).pipe(
			switchMap(() => listRemoteBranches(projectId)),
			shareReplay(1),
			catchError((e) => {
				console.error(e);
				toasts.error(`Failed load remote branches`);
				throw e;
			})
		);
		[this.branches, this.error] = observableToStore(this.branches$);
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
