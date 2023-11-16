import { BaseBranch, Branch, RemoteBranch } from './types';
import { plainToInstance } from 'class-transformer';
import { invoke } from '$lib/backend/ipc';
import { isDelete, isInsert, type Delta } from '$lib/backend/deltas';
import type { FileContent } from '$lib/backend/files';
import {
	merge,
	switchMap,
	Observable,
	shareReplay,
	catchError,
	BehaviorSubject,
	of,
	debounceTime,
	combineLatestWith
} from 'rxjs';

export class VirtualBranchService {
	branches$: Observable<Branch[]>;
	branchesError$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(
		projectId: string,
		deltas$: Observable<any>,
		sessionId$: Observable<string>,
		head$: Observable<any>,
		baseBranch$: Observable<any>
	) {
		this.branches$ = merge(deltas$, sessionId$, head$, baseBranch$).pipe(
			debounceTime(100), // TODO: Remove this when we subscribe to vbranches
			combineLatestWith(this.reload$),
			switchMap(() => {
				console.log('reloading branches');
				return listVirtualBranches({ projectId });
			}),
			catchError((e) => {
				this.branchesError$.next(e);
				return of([]);
			}),
			combineLatestWith(sessionId$),
			switchMap(
				([branches, sessionId]) =>
					new Observable<Branch[]>((subscriber) => {
						subscriber.next(branches);
						withFileContent(projectId, sessionId, branches).then((branches) =>
							subscriber.next(branches)
						);
					})
			)
		);
	}

	reload() {
		console.log('force relaod');
		this.reload$.next();
	}
}

export function getRemoteBranchesObs(
	projectId: string,
	fetches$: Observable<void>,
	head$: Observable<any>,
	baseBranch$: Observable<any>
) {
	return merge(fetches$, head$, baseBranch$).pipe(
		switchMap(() => getRemoteBranchesData({ projectId })),
		shareReplay(1)
	);
}

export class BaseBranchService {
	base$: Observable<BaseBranch | null>;
	error$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(projectId: string, fetches$: Observable<void>, head$: Observable<string>) {
		this.base$ = merge(fetches$, head$, this.reload$).pipe(
			switchMap(() => getBaseBranch({ projectId })),
			catchError((e) => {
				this.error$.next(e);
				throw e;
			}),
			shareReplay(1)
		);
	}

	reload() {
		this.reload$.next();
	}
}

export async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	const result = plainToInstance(Branch, await invoke<any[]>('list_virtual_branches', params));
	result.forEach((branch) => {
		branch.files.sort((a) => (a.conflicted ? -1 : 0));
		branch.isMergeable = invoke<boolean>('can_apply_virtual_branch', {
			projectId: params.projectId,
			branchId: branch.id
		});
	});
	return result;
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

export async function getRemoteBranches(projectId: string | undefined) {
	if (!projectId) return [];
	return await invoke<Array<string>>('git_remote_branches', { projectId });
}

async function getBaseBranch(params: { projectId: string }): Promise<BaseBranch | null> {
	const baseBranch = plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params));
	if (baseBranch) {
		// The rust code performs a fetch when get_base_branch_data is invoked
		baseBranch.fetchedAt = new Date();
		return baseBranch;
	}
	return null;
}

export async function withFileContent(
	projectId: string,
	sessionId: string,
	branches: Branch[] | undefined
) {
	if (!branches) {
		return [];
	}
	const filePaths = branches
		.map((branch) => branch.files)
		.flat()
		.map((file) => file.path);
	const sessionFiles = await invoke<Partial<Record<string, FileContent>>>('list_session_files', {
		projectId: projectId,
		sessionId: sessionId,
		paths: filePaths
	});
	const sessionDeltas = await invoke<Partial<Record<string, Delta[]>>>('list_deltas', {
		projectId: projectId,
		sessionId: sessionId,
		paths: filePaths
	});
	const branchesWithContnent = branches.map((branch) => {
		branch.files.map((file) => {
			const contentAtSessionStart = sessionFiles[file.path];
			const ds = sessionDeltas[file.path] || [];
			if (contentAtSessionStart?.type === 'utf8') {
				file.content = applyDeltas(contentAtSessionStart.value, ds);
			} else {
				file.content = applyDeltas('', ds);
			}
		});
		return branch;
	});
	return branchesWithContnent;
}

function applyDeltas(text: string, ds: Delta[]) {
	if (!ds) return text;
	const operations = ds.flatMap((delta) => delta.operations);
	operations.forEach((operation) => {
		if (isInsert(operation)) {
			text =
				text.slice(0, operation.insert[0]) + operation.insert[1] + text.slice(operation.insert[0]);
		} else if (isDelete(operation)) {
			text =
				text.slice(0, operation.delete[0]) + text.slice(operation.delete[0] + operation.delete[1]);
		}
	});
	return text;
}
