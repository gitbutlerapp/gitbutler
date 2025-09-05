import { branchReviewListingTable } from '$lib/branches/branchReviewListingsSlice';
import { BRANCH_SERVICE } from '$lib/branches/branchService';
import { branchTable } from '$lib/branches/branchesSlice';
import {
	branchReviewListingKey,
	BranchStatus,
	type Branch,
	type LoadableBranch
} from '$lib/branches/types';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import { APP_STATE } from '$lib/redux/store.svelte';
import { inject } from '@gitbutler/core/context';
import type { Loadable } from '$lib/network/types';
import type { Reactive } from '$lib/storeUtils';

/** Returns a 2D List of branches. Branches grouped in a sub-array are stack*/
export function getBranchReviewsForRepository(
	ownerSlug: string,
	projectSlug: string,
	status: BranchStatus = BranchStatus.All,
	inView?: InView
): Reactive<Loadable<Branch[][]>> {
	const appState = inject(APP_STATE);
	const branchService = inject(BRANCH_SERVICE);

	const branchReviewsInterest = branchService.getBranchesInterest(ownerSlug, projectSlug, status);
	registerInterest(branchReviewsInterest, inView);

	const branchListing = $derived(
		branchReviewListingTable.selectors.selectById(
			appState.branchReviewListings,
			branchReviewListingKey(ownerSlug, projectSlug, status)
		)
	);

	const branchReviews = $derived.by(() => {
		if (!isFound(branchListing)) return branchListing as Loadable<Branch[][]>;

		const groupedBranches = new Map<string, Branch[]>();

		branchListing.value.forEach((branchId) => {
			const loadableBranch = branchTable.selectors.selectById(appState.branches, branchId);

			if (!isFound(loadableBranch) || loadableBranch.value.status === BranchStatus.Previous) {
				return;
			}
			const branch = loadableBranch.value;

			const previouslyFoundBranches = groupedBranches.get(branch.stackId) || [];
			previouslyFoundBranches.push(branch);
			previouslyFoundBranches.sort((a, b) => b.stackOrder - a.stackOrder);
			groupedBranches.set(branch.stackId, previouslyFoundBranches);
		});

		return {
			value: [...groupedBranches.values()].sort((a, b) => {
				return new Date(b[0]!.updatedAt).getTime() - new Date(a[0]!.updatedAt).getTime();
			}),
			status: 'found'
		} as Loadable<Branch[][]>;
	});

	return {
		get current() {
			return branchReviews;
		}
	};
}

export function getBranchReview(
	uuid: string,
	inView?: InView
): Reactive<LoadableBranch | undefined> {
	const branchService = inject(BRANCH_SERVICE);
	const appState = inject(APP_STATE);
	const branchReviewInterest = branchService.getBranchInterest(uuid);
	registerInterest(branchReviewInterest, inView);

	const branchReview = $derived(branchTable.selectors.selectById(appState.branches, uuid));

	return {
		get current() {
			return branchReview;
		}
	};
}
