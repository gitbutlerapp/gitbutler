import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadablePatch } from '$lib/branches/types';

const patchesAdapter = createEntityAdapter<LoadablePatch, LoadablePatch['id']>({
	selectId: (patch: LoadablePatch) => patch.id
});

const patchesSlice = createSlice({
	name: 'patches',
	initialState: patchesAdapter.getInitialState(),
	reducers: {
		addPatch: patchesAdapter.addOne,
		addPatches: patchesAdapter.addMany,
		removePatch: patchesAdapter.removeOne,
		removePatches: patchesAdapter.removeMany,
		upsertPatch: loadableUpsert(patchesAdapter),
		upsertPatches: loadableUpsertMany(patchesAdapter)
	}
});

export const patchesReducer = patchesSlice.reducer;

export const patchesSelectors = patchesAdapter.getSelectors();
export const { addPatch, addPatches, removePatch, removePatches, upsertPatch, upsertPatches } =
	patchesSlice.actions;
