import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadablePatchCommit } from '$lib/patches/types';

const patchCommitsAdapter = createEntityAdapter<LoadablePatchCommit, LoadablePatchCommit['id']>({
	selectId: (patch: LoadablePatchCommit) => patch.id
});

const patchCommitsSlice = createSlice({
	name: 'patches',
	initialState: patchCommitsAdapter.getInitialState(),
	reducers: {
		addPatchCommit: patchCommitsAdapter.addOne,
		addPatchCommits: patchCommitsAdapter.addMany,
		removePatchCommit: patchCommitsAdapter.removeOne,
		removePatchCommits: patchCommitsAdapter.removeMany,
		upsertPatchCommit: loadableUpsert(patchCommitsAdapter),
		upsertPatchCommits: loadableUpsertMany(patchCommitsAdapter)
	}
});

export const patchCommitsReducer = patchCommitsSlice.reducer;

export const patchCommitsSelector = patchCommitsAdapter.getSelectors();
export const {
	addPatchCommit,
	addPatchCommits,
	removePatchCommit,
	removePatchCommits,
	upsertPatchCommit,
	upsertPatchCommits
} = patchCommitsSlice.actions;
