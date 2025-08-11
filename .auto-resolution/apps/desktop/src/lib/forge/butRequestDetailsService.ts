import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import type { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';

export class ButRequestDetailsService {
	constructor(
		private readonly cloudBranchService: CloudBranchService,
		private readonly latestBranchLookupService: LatestBranchLookupService
	) {}

	setDetails(reviewId: string, title: string, description: string) {
		const key = this.getStorageKey(reviewId);

		const serializedData = JSON.stringify({ title, description });

		localStorage.setItem(key, serializedData);
	}

	// This should only ever be called if a butler request exists on the server side.
	async updateDetails(ownerSlug: string, projectSlug: string, reviewId: string) {
		const key = this.getStorageKey(reviewId);
		const serializedData = localStorage.getItem(key);
		if (!serializedData) return;
		const data = JSON.parse(serializedData) as { title: string; description: string };

		const branch = await this.latestBranchLookupService.getBranch(ownerSlug, projectSlug, reviewId);

		if (!branch) return;

		await this.cloudBranchService.updateBranch(branch.uuid, {
			title: data.title,
			description: data.description
		});

		localStorage.removeItem(key);
	}

	private getStorageKey(reviewId: string): string {
		return `ButRequestDetailsService-${reviewId}`;
	}
}
