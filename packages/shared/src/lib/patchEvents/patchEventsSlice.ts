import { loadableUpsert, loadableUpsertMany } from '$lib/network/loadable';
import { type LoadablePatchEventChannel } from '$lib/patchEvents/types';
import { createEntityAdapter, createSlice } from '@reduxjs/toolkit';

const patchEventsAdapter = createEntityAdapter<
	LoadablePatchEventChannel,
	LoadablePatchEventChannel['id']
>({
	selectId: (patchEvent) => patchEvent.id
});

const patchEventsSlice = createSlice({
	name: 'patchEvents',
	initialState: patchEventsAdapter.getInitialState(),
	reducers: {
		addPatchEvent: patchEventsAdapter.addOne,
		addPatchEvents: patchEventsAdapter.addMany,
		removePatchEvent: patchEventsAdapter.removeOne,
		removePatchEvents: patchEventsAdapter.removeMany,
		upsertPatchEvent: loadableUpsert(patchEventsAdapter),
		upsertPatchEvents: loadableUpsertMany(patchEventsAdapter)
	}
});

export const patchEventsReducer = patchEventsSlice.reducer;

export const patchEventsSelectors = patchEventsAdapter.getSelectors();
export const {
	addPatchEvent,
	addPatchEvents,
	removePatchEvent,
	removePatchEvents,
	upsertPatchEvent,
	upsertPatchEvents
} = patchEventsSlice.actions;
