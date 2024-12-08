import { derived, type Readable } from 'svelte/store';
import type { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
import type { CloudBranchesService } from '@gitbutler/shared/cloud/stacks/service';
import type { CloudBranch } from '@gitbutler/shared/cloud/types';

/**
 * This service is responsible for integrating the client side oplog
 * manipulation with actual cloud patch stack creation.
 */
export class CloudBranchCreationService {
	canCreateBranch: Readable<boolean>;

	constructor(
		private readonly syncedSnapshotService: SyncedSnapshotService,
		private readonly cloudBranchesService: CloudBranchesService
	) {
		this.canCreateBranch = derived(
			[this.syncedSnapshotService.canTakeSnapshot, this.cloudBranchesService.canCreateBranch],
			([canTakeSnapshot, canCreateBranch]) => {
				return canTakeSnapshot && canCreateBranch;
			}
		);
	}

	async createBranch(branchId: string): Promise<CloudBranch> {
		const oplogSha = await this.syncedSnapshotService.takeSyncedSnapshot();
		const cloudBranch = await this.cloudBranchesService.createBranch(branchId, oplogSha);
		return cloudBranch;
	}
}
