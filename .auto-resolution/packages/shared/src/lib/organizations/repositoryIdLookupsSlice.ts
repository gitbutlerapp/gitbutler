import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadableRepositoryId } from '$lib/organizations/types';

const repositoryIdLookupsAdapter = createEntityAdapter<
	LoadableRepositoryId,
	LoadableRepositoryId['id']
>({
	selectId: (project: LoadableRepositoryId) => project.id
});

const repositoryIdLookupsSlice = createSlice({
	name: 'repositoryIds',
	initialState: repositoryIdLookupsAdapter.getInitialState(),
	reducers: {
		addRepositoryId: repositoryIdLookupsAdapter.addOne,
		addRepositoryIds: repositoryIdLookupsAdapter.addMany,
		removeRepositoryId: repositoryIdLookupsAdapter.removeOne,
		removeRepositoryIds: repositoryIdLookupsAdapter.removeMany,
		upsertRepositoryId: loadableUpsert(repositoryIdLookupsAdapter),
		upsertRepositoryIds: loadableUpsertMany(repositoryIdLookupsAdapter)
	}
});

export const repositoryIdLookupsReducer = repositoryIdLookupsSlice.reducer;

export const repositoryIdLookupsSelectors = repositoryIdLookupsAdapter.getSelectors();
export const {
	addRepositoryId,
	addRepositoryIds,
	removeRepositoryId,
	removeRepositoryIds,
	upsertRepositoryId,
	upsertRepositoryIds
} = repositoryIdLookupsSlice.actions;
