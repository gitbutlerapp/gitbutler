import { BranchReviewService } from '$lib/branchReviews/branchReviewService';
import { branchReviewsSelectors } from '$lib/branchReviews/branchReviewsSlice';
import { BranchReviewStatus, type BranchReview } from '$lib/branchReviews/types';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import type { AppBranchReviewsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getBranchReviews(
	appState: AppBranchReviewsState,
	branchReviewService: BranchReviewService,
	repositoryId: string,
	status: BranchReviewStatus = BranchReviewStatus.All,
	inView?: InView
): Reactive<BranchReview[]> {
	const branchReviewsInterest = branchReviewService.getBranchReviewsInterest(repositoryId, status);
	registerInterest(branchReviewsInterest, inView);

	const branchReviews = $derived(branchReviewsSelectors.selectAll(appState.branchReviews));

	return {
		get current() {
			return branchReviews;
		}
	};
}

export function getBranchReview(
	appState: AppBranchReviewsState,
	branchReviewService: BranchReviewService,
	repositoryId: string,
	branchId: string,
	inView?: InView
): Reactive<BranchReview | undefined> {
	const branchReviewInterest = branchReviewService.getBranchReviewInterest(repositoryId, branchId);
	registerInterest(branchReviewInterest, inView);

	const branchReview = $derived(
		branchReviewsSelectors.selectById(appState.branchReviews, branchId)
	);

	return {
		get current() {
			return branchReview;
		}
	};
}
