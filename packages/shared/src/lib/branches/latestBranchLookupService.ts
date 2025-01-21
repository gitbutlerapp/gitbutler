import { addBranchUuid, upsertBranchUuid } from '$lib/branches/latestBranchLookupSlice';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_REGULAR } from '$lib/polling';
import type { ApiBranch } from '$lib/branches/types';
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
				this.appDispatch.dispatch(addBranchUuid({ status: 'loading', id: branchId }));

				try {
					const branch = await this.httpClient.get<ApiBranch>(
						`patch_stack/${ownerSlug}/{projectSlug}/${branchId}`
					);

					this.appDispatch.dispatch(
						upsertBranchUuid({
							status: 'found',
							id: branchId,
							value: branch.uuid
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertBranchUuid(errorToLoadable(error, branchId)));
				}
			})
			.createInterest();
	}
}
