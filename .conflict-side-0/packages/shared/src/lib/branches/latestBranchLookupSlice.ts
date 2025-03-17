import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableBranchUuid } from '$lib/branches/types';

const latestBranchLookupsAdapter = createEntityAdapter<
	LoadableBranchUuid,
	LoadableBranchUuid['id']
>({
	selectId: (project: LoadableBranchUuid) => project.id
});

const latestBranchLookupsSlice = createSlice({
	name: 'repositoryIds',
	initialState: latestBranchLookupsAdapter.getInitialState(),
	reducers: {
		addBranchUuid: latestBranchLookupsAdapter.addOne,
		addBranchUuids: latestBranchLookupsAdapter.addMany,
		removeBranchUuid: latestBranchLookupsAdapter.removeOne,
		removeBranchUuids: latestBranchLookupsAdapter.removeMany,
		upsertBranchUuid: loadableUpsert(latestBranchLookupsAdapter),
		upsertBranchUuids: loadableUpsertMany(latestBranchLookupsAdapter)
	}
});

export const latestBranchLookupsReducer = latestBranchLookupsSlice.reducer;

export const latestBranchLookupsSelectors = latestBranchLookupsAdapter.getSelectors();
export const {
	addBranchUuid,
	addBranchUuids,
	removeBranchUuid,
	removeBranchUuids,
	upsertBranchUuid,
	upsertBranchUuids
} = latestBranchLookupsSlice.actions;
