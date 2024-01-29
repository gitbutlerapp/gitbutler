import { invoke } from '$lib/backend/ipc';
import * as toasts from '$lib/utils/toasts';
import { RemoteBranch } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';
import {
	BehaviorSubject,
	Observable,
	catchError,
	combineLatest,
	map,
	of,
	shareReplay,
	switchMap
} from 'rxjs';

export class RemoteBranchService {
	branches$: Observable<RemoteBranch[]>;
	branchesError$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(
		projectId: string,
		fetches$: Observable<any>,
		head$: Observable<any>,
		baseBranch$: Observable<any>
	) {
		this.branches$ = combineLatest([baseBranch$, this.reload$, head$, fetches$]).pipe(
			switchMap(() => getRemoteBranchesData({ projectId })),
			map((branches) => branches.filter((b) => b.ahead != 0)),
			shareReplay(1),
			catchError((e) => {
				console.log(e);
				this.branchesError$.next(e);
				toasts.error(`Failed load remote branches`);
				return of([]);
			})
		);
	}

	reload() {
		this.reload$.next();
	}
}

export async function getRemoteBranchesData(params: {
	projectId: string;
}): Promise<RemoteBranch[]> {
	const branches = plainToInstance(
		RemoteBranch,
		await invoke<any[]>('list_remote_branches', params)
	);

	return branches;
}
