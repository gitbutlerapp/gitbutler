import { branchReviewListingsSelectors } from '$lib/branches/branchReviewListingsSlice';
import { branchesSelectors } from '$lib/branches/branchesSlice';
import { BranchStatus, toCombineSlug, type Branch, type LoadableBranch } from '$lib/branches/types';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import type { BranchService } from '$lib/branches/branchService';
import type { Loadable } from '$lib/network/types';
import type { AppBranchesState, AppBranchReviewListingsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

/** Returns a 2D List of branches. Branches grouped in a sub-array are stack*/
export function getBranchReviewsForRepository(
	appState: AppBranchesState & AppBranchReviewListingsState,
	branchService: BranchService,
	ownerSlug: string,
	projectSlug: string,
	status: BranchStatus = BranchStatus.All,
	inView?: InView
): Reactive<Loadable<Branch[][]>> {
	const branchReviewsInterest = branchService.getBranchesInterest(ownerSlug, projectSlug, status);
	registerInterest(branchReviewsInterest, inView);

	const branchListing = $derived(
		branchReviewListingsSelectors.selectById(
			appState.branchReviewListings,
			toCombineSlug(ownerSlug, projectSlug)
		)
	);

	const branchReviews = $derived.by(() => {
		if (!isFound(branchListing)) return branchListing as Loadable<Branch[][]>;

		const groupedBranches = new Map<string, Branch[]>();

		branchListing.value.forEach((branchId) => {
			const loadableBranch = branchesSelectors.selectById(appState.branches, branchId);

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
	appState: AppBranchesState,
	branchService: BranchService,
	uuid: string,
	inView?: InView
): Reactive<LoadableBranch | undefined> {
	const branchReviewInterest = branchService.getBranchInterest(uuid);
	registerInterest(branchReviewInterest, inView);

	const branchReview = $derived(branchesSelectors.selectById(appState.branches, uuid));

	return {
		get current() {
			return branchReview;
		}
	};
}
