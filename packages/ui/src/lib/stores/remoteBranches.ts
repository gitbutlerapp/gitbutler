import { getRemoteBranchesData } from '$lib/vbranches/branchStoresCache';
import type { RemoteBranch } from '$lib/vbranches/types';
import {
	BehaviorSubject,
	Observable,
	catchError,
	combineLatestWith,
	merge,
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
		this.branches$ = merge(fetches$, head$, baseBranch$).pipe(
			combineLatestWith(this.reload$),
			switchMap(() => getRemoteBranchesData({ projectId })),
			shareReplay(1),
			catchError((e) => {
				this.branchesError$.next(e);
				return of([]);
			})
		);
	}

	reload() {
		this.reload$.next();
	}
}
