import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
import { getForgePrService } from '$lib/forge/interface/forgePrService';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { PatchSeries } from '$lib/branches/branch';
import type { DetailedPullRequest } from '$lib/forge/interface/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function getPr(branch: Reactive<PatchSeries>): Reactive<DetailedPullRequest | undefined> {
	const prService = $derived(readableToReactive(getForgePrService()));
	const forgeListing = $derived(readableToReactive(getForgeListingService()));
	const prs = $derived(readableToReactive(forgeListing.current?.prs));

	const upstreamName = $derived(branch.current.upstreamReference ? branch.current.name : undefined);
	const listedPr = $derived(prs.current?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(branch.current.prNumber || listedPr?.number);

	const prMonitor = $derived(prNumber ? prService.current?.prMonitor(prNumber) : undefined);
	const pr = $derived(readableToReactive(prMonitor?.pr));

	return {
		get current() {
			return pr.current;
		}
	};
}
