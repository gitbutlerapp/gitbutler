import { LatestBranchLookupService } from '$lib/branches/latestBranchLookupService';
import { latestBranchLookupsSelectors } from '$lib/branches/latestBranchLookupSlice';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import type { LoadableBranchUuid } from '$lib/branches/types';
import type { AppLatestBranchLookupsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function lookupLatestBranchUuid(
	appState: AppLatestBranchLookupsState,
	latestBranchLookupService: LatestBranchLookupService,
	repositoryId: string,
	branchId: string,
	inView?: InView
): Reactive<LoadableBranchUuid | undefined> {
	registerInterest(latestBranchLookupService.getBranchUuidInterest(repositoryId, branchId), inView);
	const branchUuid = $derived(
		latestBranchLookupsSelectors.selectById(appState.latestBranchLookups, branchId)
	);

	return {
		get current() {
			return branchUuid;
		}
	};
}
