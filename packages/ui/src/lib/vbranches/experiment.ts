import { invoke, listen } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';
import { Branch } from './types';
import { Observable, from, concat, shareReplay } from 'rxjs';

export function vbExperiment(projectId: string) {
	return concat(
		from(listVirtualBranches({ projectId })),
		new Observable<Branch[]>((subscriber) => {
			return subscribeToVirtualBranches(projectId, (branches) => subscriber.next(branches));
		})
	).pipe(shareReplay(1));
}

function subscribeToVirtualBranches(projectId: string, callback: (branches: Branch[]) => void) {
	return listen<Branch[]>(`project://${projectId}/virtual-branches`, (event) => {
		console.log('GOT VBRANCH', event.payload);
		callback(event.payload);
	});
}

// Also: lets not attach file content to the branche eagerly, at least not until its needed
async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	const result = plainToInstance(Branch, await invoke<any[]>('list_virtual_branches', params));
	result.forEach((branch) => {
		branch.files.sort((a) => (a.conflicted ? -1 : 0));
		// this is somehow pointless
		branch.isMergeable = invoke<boolean>('can_apply_virtual_branch', {
			projectId: params.projectId,
			branchId: branch.id
		});
	});
	return result;
}
