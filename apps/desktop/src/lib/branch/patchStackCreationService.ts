import { derived, type Readable } from 'svelte/store';
import type { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
import type {
	CloudPatchStack,
	CloudPatchStacksService
} from '@gitbutler/shared/cloud/stacks/service';

/**
 * This service is responsible for integrating the client side oplog
 * manipulation with actual cloud patch stack creation.
 */
export class PatchStackCreationService {
	canCreatePatchStack: Readable<boolean>;

	constructor(
		private readonly syncedSnapshotService: SyncedSnapshotService,
		private readonly cloudPatchStacksService: CloudPatchStacksService
	) {
		this.canCreatePatchStack = derived(
			[
				this.syncedSnapshotService.canTakeSnapshot,
				this.cloudPatchStacksService.canCreatePatchStack
			],
			([canTakeSnapshot, canCreatePatchStack]) => {
				return canTakeSnapshot && canCreatePatchStack;
			}
		);
	}

	async createPatchStack(branchId: string): Promise<CloudPatchStack> {
		const oplogSha = await this.syncedSnapshotService.takeSyncedSnapshot();
		const patchStack = await this.cloudPatchStacksService.createPatchStack(branchId, oplogSha);
		return patchStack;
	}
}
