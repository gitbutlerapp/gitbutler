import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableBranch } from '$lib/branches/types';

const branchesAdapter = createEntityAdapter<LoadableBranch, LoadableBranch['id']>({
	selectId: (branch: LoadableBranch) => branch.id
});

const branchesSlice = createSlice({
	name: 'branches',
	initialState: branchesAdapter.getInitialState(),
	reducers: {
		addBranch: branchesAdapter.addOne,
		addBranches: branchesAdapter.addMany,
		removeBranch: branchesAdapter.removeOne,
		removeBranches: branchesAdapter.removeMany,
		upsertBranch: loadableUpsert(branchesAdapter),
		upsertBranches: loadableUpsertMany(branchesAdapter)
	}
});

export const branchesReducer = branchesSlice.reducer;

export const branchesSelectors = branchesAdapter.getSelectors();
export const {
	addBranch,
	addBranches,
	removeBranch,
	removeBranches,
	upsertBranch,
	upsertBranches
} = branchesSlice.actions;
