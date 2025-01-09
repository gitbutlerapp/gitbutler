import { derived, type Readable } from 'svelte/store';
import type { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';

/**
 * This service is responsible for integrating the client side oplog
 * manipulation with actual cloud patch stack creation.
 */
export class CloudBranchCreationService {
	canCreateBranch: Readable<boolean>;

	constructor(
		private readonly syncedSnapshotService: SyncedSnapshotService
		// private readonly cloudBranchesService: CloudBranchesService
	) {
		this.canCreateBranch = derived(
			[this.syncedSnapshotService.canTakeSnapshot],
			([canTakeSnapshot]) => {
				return canTakeSnapshot;
			}
		);
	}

	async createBranch(_branchId: string): Promise<void> {
		const _oplogSha = await this.syncedSnapshotService.takeSyncedSnapshot();
		// const cloudBranch = await this.cloudBranchesService.createBranch(branchId, oplogSha);
		// return cloudBranch;
	}
}
