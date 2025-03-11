import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableBranchReviewListing } from '$lib/branches/types';

const branchReviewListingsAdapter = createEntityAdapter<
	LoadableBranchReviewListing,
	LoadableBranchReviewListing['id']
>({
	selectId: (branchReviewListing: LoadableBranchReviewListing) => branchReviewListing.id
});

const branchReviewListingsSlice = createSlice({
	name: 'branchReviewListings',
	initialState: branchReviewListingsAdapter.getInitialState(),
	reducers: {
		addBranchReviewListing: branchReviewListingsAdapter.addOne,
		addBranchReviewListings: branchReviewListingsAdapter.addMany,
		removeBranchReviewListing: branchReviewListingsAdapter.removeOne,
		removeBranchReviewListings: branchReviewListingsAdapter.removeMany,
		upsertBranchReviewListing: loadableUpsert(branchReviewListingsAdapter),
		upsertBranchReviewListings: loadableUpsertMany(branchReviewListingsAdapter)
	}
});

export const branchReviewListingsReducer = branchReviewListingsSlice.reducer;

export const branchReviewListingsSelectors = branchReviewListingsAdapter.getSelectors();
export const {
	addBranchReviewListing,
	addBranchReviewListings,
	removeBranchReviewListing,
	removeBranchReviewListings,
	upsertBranchReviewListing,
	upsertBranchReviewListings
} = branchReviewListingsSlice.actions;
