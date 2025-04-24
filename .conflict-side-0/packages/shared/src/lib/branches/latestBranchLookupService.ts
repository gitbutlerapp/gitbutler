import { latestBranchLookupTable } from '$lib/branches/latestBranchLookupSlice';
import { apiToBranch, type ApiBranch, type Branch } from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class LatestBranchLookupService {
	private readonly branchLookupInterests = new InterestStore<{ branchId: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getBranchUuidInterest(ownerSlug: string, projectSlug: string, branchId: string): Interest {
		return this.branchLookupInterests
			.findOrCreateSubscribable({ branchId }, async () => {
				this.appDispatch.dispatch(
					latestBranchLookupTable.addOne({ status: 'loading', id: branchId })
				);

				try {
					const branch = await this.httpClient.get<ApiBranch>(
						`patch_stack/${ownerSlug}/${projectSlug}/branch/${branchId}`
					);

					this.appDispatch.dispatch(
						latestBranchLookupTable.upsertOne({
							status: 'found',
							id: branchId,
							value: branch.uuid
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						latestBranchLookupTable.addOne(errorToLoadable(error, branchId))
					);
				}
			})
			.createInterest();
	}

	async getBranch(
		ownerSlug: string,
		projectSlug: string,
		branchId: string
	): Promise<Branch | undefined> {
		try {
			const branch = await this.httpClient.get<ApiBranch>(
				`patch_stack/${ownerSlug}/${projectSlug}/branch/${branchId}`
			);

			this.appDispatch.dispatch(
				latestBranchLookupTable.upsertOne({
					status: 'found',
					id: branchId,
					value: branch.uuid
				})
			);

			return apiToBranch(branch);
		} catch (_: unknown) {
			/* empty */
		}
	}

	async refreshBranchUuid(branchId: string) {
		await this.branchLookupInterests.invalidate({ branchId });
	}
}
