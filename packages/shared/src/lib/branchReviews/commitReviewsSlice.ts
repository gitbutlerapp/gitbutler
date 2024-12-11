import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { CommitReview } from '$lib/branchReviews/types';

const commitReviewsAdapter = createEntityAdapter({
	selectId: (commitReview: CommitReview) => commitReview.changeId,
	sortComparer: (a: CommitReview, b: CommitReview) => a.title.localeCompare(b.title)
});

const commitReviewsSlice = createSlice({
	name: 'commitReviews',
	initialState: commitReviewsAdapter.getInitialState(),
	reducers: {
		addCommitReview: commitReviewsAdapter.addOne,
		addCommitReviews: commitReviewsAdapter.addMany,
		removeCommitReview: commitReviewsAdapter.removeOne,
		removeCommitReviews: commitReviewsAdapter.removeMany,
		upsertCommitReview: commitReviewsAdapter.upsertOne,
		upsertCommitReviews: commitReviewsAdapter.upsertMany
	}
});

export const commitReviewsReducer = commitReviewsSlice.reducer;

export const commitReviewsSelectors = commitReviewsAdapter.getSelectors();
export const {
	addCommitReview,
	addCommitReviews,
	removeCommitReview,
	removeCommitReviews,
	upsertCommitReview,
	upsertCommitReviews
} = commitReviewsSlice.actions;
