import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { BranchReview } from '$lib/branchReviews/types';

const branchReviewsAdapter = createEntityAdapter({
	selectId: (branchReview: BranchReview) => branchReview.branchId,
	sortComparer: (a: BranchReview, b: BranchReview) => a.title.localeCompare(b.title)
});

const branchReviewsSlice = createSlice({
	name: 'branchReviews',
	initialState: branchReviewsAdapter.getInitialState(),
	reducers: {
		addBranchReview: branchReviewsAdapter.addOne,
		addBranchReviews: branchReviewsAdapter.addMany,
		removeBranchReview: branchReviewsAdapter.removeOne,
		removeBranchReviews: branchReviewsAdapter.removeMany,
		upsertBranchReview: branchReviewsAdapter.upsertOne,
		upsertBranchReviews: branchReviewsAdapter.upsertMany
	}
});

export const branchReviewsReducer = branchReviewsSlice.reducer;

export const branchReviewsSelectors = branchReviewsAdapter.getSelectors();
export const {
	addBranchReview,
	addBranchReviews,
	removeBranchReview,
	removeBranchReviews,
	upsertBranchReview,
	upsertBranchReviews
} = branchReviewsSlice.actions;
