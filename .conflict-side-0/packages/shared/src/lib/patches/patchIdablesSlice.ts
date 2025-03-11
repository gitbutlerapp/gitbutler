import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { LoadablePatchIdable } from '$lib/patches/types';

const patchIdablesAdapter = createEntityAdapter<LoadablePatchIdable, LoadablePatchIdable['id']>({
	selectId: (patch: LoadablePatchIdable) => patch.id
});

const patchIdablesSlice = createSlice({
	name: 'patches',
	initialState: patchIdablesAdapter.getInitialState(),
	reducers: {
		addPatchIdable: patchIdablesAdapter.addOne,
		addPatchIdables: patchIdablesAdapter.addMany,
		removePatchIdable: patchIdablesAdapter.removeOne,
		removePatchIdables: patchIdablesAdapter.removeMany,
		upsertPatchIdable: loadableUpsert(patchIdablesAdapter),
		upsertPatchIdables: loadableUpsertMany(patchIdablesAdapter)
	}
});

export const patchIdablesReducer = patchIdablesSlice.reducer;

export const patchIdablesSelector = patchIdablesAdapter.getSelectors();
export const {
	addPatchIdable,
	addPatchIdables,
	removePatchIdable,
	removePatchIdables,
	upsertPatchIdable,
	upsertPatchIdables
} = patchIdablesSlice.actions;
